use crate::error::{MediaError, Result};
use crate::types::{KeyframeInfo, StreamId};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Snapshot 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotMode {
    /// 自动选择（优先 keyframe，降级到 decode）
    Auto,
    /// 仅 Keyframe 快照（低延迟、低成本）
    Keyframe,
    /// Decode 快照（高质量、可缩放/水印）
    Decode,
}

impl Default for SnapshotMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// Snapshot 请求
#[derive(Debug, Clone)]
pub struct SnapshotRequest {
    pub stream_id: StreamId,
    pub mode: SnapshotMode,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Snapshot 结果
#[derive(Debug, Clone)]
pub struct SnapshotResult {
    pub data: Bytes,
    pub mode_used: SnapshotMode,
    pub timestamp: DateTime<Utc>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Snapshot 编排器（协议无关）
/// 
/// 职责：
/// 1. 管理 keyframe cache
/// 2. 提供 keyframe/decode 双模式 snapshot
/// 3. 实现 auto 模式的降级策略
pub struct SnapshotOrchestrator {
    keyframe_dir: PathBuf,
    cache: Arc<RwLock<SnapshotCache>>,
    decoder: Option<Arc<dyn SnapshotDecoder>>,
}

impl SnapshotOrchestrator {
    pub fn new(keyframe_dir: PathBuf) -> Self {
        Self {
            keyframe_dir,
            cache: Arc::new(RwLock::new(SnapshotCache::new())),
            decoder: None,
        }
    }

    pub fn with_decoder(mut self, decoder: Arc<dyn SnapshotDecoder>) -> Self {
        self.decoder = Some(decoder);
        self
    }

    /// 处理视频样本并提取关键帧
    pub async fn process_keyframe(
        &self,
        stream_id: &StreamId,
        data: &[u8],
        timestamp: DateTime<Utc>,
    ) -> Result<Option<KeyframeInfo>> {
        if !Self::is_keyframe(data) {
            return Ok(None);
        }

        let keyframe_path = self.keyframe_path(stream_id, timestamp);
        if let Some(parent) = keyframe_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&keyframe_path, data).await?;

        let info = KeyframeInfo {
            stream_id: stream_id.clone(),
            file_path: keyframe_path.to_string_lossy().to_string(),
            timestamp,
            size: data.len(),
        };

        let mut cache = self.cache.write().await;
        cache.update(stream_id.clone(), info.clone());

        Ok(Some(info))
    }

    /// 获取 snapshot
    pub async fn get_snapshot(&self, req: SnapshotRequest) -> Result<SnapshotResult> {
        match req.mode {
            SnapshotMode::Keyframe => self.get_keyframe_snapshot(&req.stream_id).await,
            SnapshotMode::Decode => self.get_decode_snapshot(&req).await,
            SnapshotMode::Auto => {
                // 优先尝试 keyframe，失败则降级到 decode
                match self.get_keyframe_snapshot(&req.stream_id).await {
                    Ok(result) => Ok(result),
                    Err(_) => self.get_decode_snapshot(&req).await,
                }
            }
        }
    }

    async fn get_keyframe_snapshot(&self, stream_id: &StreamId) -> Result<SnapshotResult> {
        let cache = self.cache.read().await;
        let info = cache
            .get(stream_id)
            .ok_or_else(|| MediaError::SnapshotNotAvailable(stream_id.to_string()))?;

        let data = tokio::fs::read(&info.file_path).await.map_err(|e| {
            MediaError::SnapshotNotAvailable(format!("Failed to read keyframe: {}", e))
        })?;

        Ok(SnapshotResult {
            data: Bytes::from(data),
            mode_used: SnapshotMode::Keyframe,
            timestamp: info.timestamp,
            width: None,
            height: None,
        })
    }

    async fn get_decode_snapshot(&self, req: &SnapshotRequest) -> Result<SnapshotResult> {
        let decoder = self.decoder.as_ref().ok_or_else(|| {
            MediaError::SnapshotNotAvailable("Decoder not configured".to_string())
        })?;

        // 先获取 keyframe 作为解码源
        let (file_path, timestamp) = {
            let cache = self.cache.read().await;
            let info = cache
                .get(&req.stream_id)
                .ok_or_else(|| MediaError::SnapshotNotAvailable(req.stream_id.to_string()))?;
            (info.file_path.clone(), info.timestamp)
        };

        let keyframe_data = tokio::fs::read(&file_path).await.map_err(|e| {
            MediaError::SnapshotNotAvailable(format!("Failed to read keyframe: {}", e))
        })?;

        // 解码并生成 JPEG
        let decoded = decoder
            .decode(&keyframe_data, req.width, req.height)
            .await?;

        Ok(SnapshotResult {
            data: decoded,
            mode_used: SnapshotMode::Decode,
            timestamp,
            width: req.width,
            height: req.height,
        })
    }

    fn keyframe_path(&self, stream_id: &StreamId, timestamp: DateTime<Utc>) -> PathBuf {
        let ts_millis = timestamp.timestamp_millis();
        self.keyframe_dir
            .join(stream_id.as_str())
            .join(format!("{}.h264", ts_millis))
    }

    fn is_keyframe(data: &[u8]) -> bool {
        // 简化的 H264 IDR 检测
        // 查找 NALU type 5 (IDR)
        let mut i = 0;
        while i < data.len() {
            // 检查 4 字节起始码 0x00000001
            if i + 4 < data.len()
                && data[i] == 0
                && data[i + 1] == 0
                && data[i + 2] == 0
                && data[i + 3] == 1
            {
                if i + 4 < data.len() {
                    let nalu_type = data[i + 4] & 0x1F;
                    if nalu_type == 5 {
                        return true;
                    }
                }
                i += 4;
            }
            // 检查 3 字节起始码 0x000001
            else if i + 3 < data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
                if i + 3 < data.len() {
                    let nalu_type = data[i + 3] & 0x1F;
                    if nalu_type == 5 {
                        return true;
                    }
                }
                i += 3;
            } else {
                i += 1;
            }
        }
        false
    }
}

/// Snapshot 缓存
struct SnapshotCache {
    latest: HashMap<StreamId, KeyframeInfo>,
}

impl SnapshotCache {
    fn new() -> Self {
        Self {
            latest: HashMap::new(),
        }
    }

    fn update(&mut self, stream_id: StreamId, info: KeyframeInfo) {
        self.latest.insert(stream_id, info);
    }

    fn get(&self, stream_id: &StreamId) -> Option<&KeyframeInfo> {
        self.latest.get(stream_id)
    }
}

/// Snapshot 解码器接口（可扩展）
#[async_trait::async_trait]
pub trait SnapshotDecoder: Send + Sync {
    async fn decode(
        &self,
        h264_data: &[u8],
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<Bytes>;
}

/// Stub 解码器（用于测试和占位）
pub struct StubDecoder;

#[async_trait::async_trait]
impl SnapshotDecoder for StubDecoder {
    async fn decode(
        &self,
        h264_data: &[u8],
        _width: Option<u32>,
        _height: Option<u32>,
    ) -> Result<Bytes> {
        // Stub: 直接返回原始数据
        // 生产环境应使用 ffmpeg/gstreamer 等解码并生成 JPEG
        Ok(Bytes::copy_from_slice(h264_data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_snapshot_orchestrator_keyframe() {
        let temp_dir = tempdir().unwrap();
        let orchestrator = SnapshotOrchestrator::new(temp_dir.path().to_path_buf());

        let stream_id = StreamId::new("test", "stream1");
        let timestamp = Utc::now();

        // 构造包含 IDR NALU 的数据
        let mut h264_data = Vec::new();
        h264_data.extend_from_slice(&[0, 0, 0, 1, 0x67]); // SPS
        h264_data.extend_from_slice(&[0, 0, 0, 1, 0x68]); // PPS
        h264_data.extend_from_slice(&[0, 0, 0, 1, 0x65]); // IDR (type 5)
        h264_data.extend_from_slice(&[0xAA; 100]);

        let result = orchestrator
            .process_keyframe(&stream_id, &h264_data, timestamp)
            .await
            .unwrap();

        assert!(result.is_some());

        let req = SnapshotRequest {
            stream_id: stream_id.clone(),
            mode: SnapshotMode::Keyframe,
            width: None,
            height: None,
        };

        let snapshot = orchestrator.get_snapshot(req).await.unwrap();
        assert_eq!(snapshot.mode_used, SnapshotMode::Keyframe);
        assert!(!snapshot.data.is_empty());
    }

    #[tokio::test]
    async fn test_snapshot_orchestrator_auto_fallback() {
        let temp_dir = tempdir().unwrap();
        let orchestrator = SnapshotOrchestrator::new(temp_dir.path().to_path_buf())
            .with_decoder(Arc::new(StubDecoder));

        let stream_id = StreamId::new("test", "stream1");
        let timestamp = Utc::now();

        let mut h264_data = Vec::new();
        h264_data.extend_from_slice(&[0, 0, 0, 1, 0x65]); // IDR
        h264_data.extend_from_slice(&[0xBB; 50]);

        orchestrator
            .process_keyframe(&stream_id, &h264_data, timestamp)
            .await
            .unwrap();

        let req = SnapshotRequest {
            stream_id: stream_id.clone(),
            mode: SnapshotMode::Auto,
            width: Some(640),
            height: Some(480),
        };

        let snapshot = orchestrator.get_snapshot(req).await.unwrap();
        assert!(matches!(
            snapshot.mode_used,
            SnapshotMode::Keyframe | SnapshotMode::Decode
        ));
    }

    #[test]
    fn test_is_keyframe_detection() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0, 0, 0, 1, 0x65]); // IDR
        assert!(SnapshotOrchestrator::is_keyframe(&data));

        let mut data2 = Vec::new();
        data2.extend_from_slice(&[0, 0, 1, 0x65]); // IDR (3-byte start code)
        assert!(SnapshotOrchestrator::is_keyframe(&data2));

        let mut data3 = Vec::new();
        data3.extend_from_slice(&[0, 0, 0, 1, 0x61]); // Non-IDR
        assert!(!SnapshotOrchestrator::is_keyframe(&data3));
    }
}
