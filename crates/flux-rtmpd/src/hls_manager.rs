use anyhow::Result;
use bytes::Bytes;
use flux_media_core::playback::{HlsGenerator, TsMuxer};
use flux_media_core::types::StreamId;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// HLS 管理器：负责将 RTMP 流转换为 HLS
pub struct HlsManager {
    generators: Arc<RwLock<HashMap<String, Arc<HlsStreamContext>>>>,
    storage_dir: PathBuf,
}

/// HLS 流上下文
pub struct HlsStreamContext {
    pub stream_id: StreamId,
    pub hls_generator: Arc<RwLock<HlsGenerator>>,
    pub ts_muxer: Arc<RwLock<TsMuxer>>,
    pub current_segment: Arc<RwLock<SegmentBuffer>>,
    pub segment_duration: u32,
    pub last_keyframe_ts: Arc<RwLock<u32>>,
}

/// 分片缓冲区
pub struct SegmentBuffer {
    pub data: Vec<Bytes>,
    pub duration: f64,
    pub start_timestamp: u32,
}

impl HlsManager {
    pub fn new(storage_dir: PathBuf) -> Self {
        Self {
            generators: Arc::new(RwLock::new(HashMap::new())),
            storage_dir,
        }
    }

    /// 注册流（开始 HLS 转换）
    pub async fn register_stream(
        &self,
        app_name: String,
        stream_key: String,
        segment_duration: u32,
    ) -> Result<()> {
        let stream_id = StreamId::new("rtmp", &format!("{}/{}", app_name, stream_key));
        let key = format!("{}/{}", app_name, stream_key);

        let hls_generator = Arc::new(RwLock::new(HlsGenerator::new(
            stream_id.clone(),
            segment_duration,
            5, // 保留 5 个分片
        )));

        let ts_muxer = Arc::new(RwLock::new(TsMuxer::new()));

        let context = HlsStreamContext {
            stream_id,
            hls_generator,
            ts_muxer,
            current_segment: Arc::new(RwLock::new(SegmentBuffer {
                data: Vec::new(),
                duration: 0.0,
                start_timestamp: 0,
            })),
            segment_duration,
            last_keyframe_ts: Arc::new(RwLock::new(0)),
        };

        let mut generators = self.generators.write().await;
        generators.insert(key.clone(), Arc::new(context));

        info!(target: "hls_manager", stream_key = %key, "HLS stream registered");
        Ok(())
    }

    /// 注销流
    pub async fn unregister_stream(&self, app_name: &str, stream_key: &str) -> Result<()> {
        let key = format!("{}/{}", app_name, stream_key);
        let mut generators = self.generators.write().await;
        generators.remove(&key);

        info!(target: "hls_manager", stream_key = %key, "HLS stream unregistered");
        Ok(())
    }

    /// 处理视频数据
    pub async fn process_video(
        &self,
        app_name: &str,
        stream_key: &str,
        data: &[u8],
        timestamp: u32,
        is_keyframe: bool,
    ) -> Result<()> {
        let key = format!("{}/{}", app_name, stream_key);
        let generators = self.generators.read().await;

        if let Some(context) = generators.get(&key) {
            // 如果是关键帧，检查是否需要切分片
            if is_keyframe {
                let last_keyframe_ts = *context.last_keyframe_ts.read().await;
                let duration_ms = if last_keyframe_ts > 0 {
                    timestamp.saturating_sub(last_keyframe_ts)
                } else {
                    0
                };

                // 如果距离上次关键帧超过分片时长，切分片
                if duration_ms >= context.segment_duration * 1000 {
                    self.finalize_segment(context).await?;
                    *context.last_keyframe_ts.write().await = timestamp;
                }
            }

            // 封装为 TS 包
            let mut ts_muxer = context.ts_muxer.write().await;
            let pts = timestamp as u64 * 90; // 转换为 90kHz 时钟
            let dts = pts;

            match ts_muxer.mux_video_pes(data, pts, dts, is_keyframe) {
                Ok(ts_packets) => {
                    let packet_count = ts_packets.len();
                    
                    // 添加到当前分片
                    let mut segment = context.current_segment.write().await;
                    if segment.data.is_empty() {
                        segment.start_timestamp = timestamp;
                    }
                    for packet in ts_packets {
                        segment.data.push(packet);
                    }
                    segment.duration = (timestamp - segment.start_timestamp) as f64 / 1000.0;

                    debug!(target: "hls_manager", 
                        stream_key = %key, 
                        ts_packets = packet_count,
                        "Video TS packets added"
                    );
                }
                Err(e) => {
                    error!(target: "hls_manager", "TS muxing error: {}", e);
                }
            }
        }

        Ok(())
    }

    /// 完成当前分片
    async fn finalize_segment(&self, context: &HlsStreamContext) -> Result<()> {
        let mut segment = context.current_segment.write().await;
        
        if segment.data.is_empty() {
            return Ok(());
        }

        let duration = segment.duration;
        let total_size: usize = segment.data.iter().map(|b| b.len()).sum();

        // 添加分片到 HLS 生成器
        let hls_generator = context.hls_generator.write().await;
        let segment_info = hls_generator.add_segment(duration, total_size).await?;

        // 保存 TS 分片到磁盘
        let stream_dir = self.storage_dir.join(context.stream_id.as_str());
        if let Err(e) = fs::create_dir_all(&stream_dir).await {
            error!(target: "hls_manager", "Failed to create stream directory: {}", e);
        } else {
            let segment_path = stream_dir.join(&segment_info.filename);
            
            // 合并所有 TS 包
            let mut ts_data = Vec::new();
            for packet in &segment.data {
                ts_data.extend_from_slice(packet);
            }

            // 写入文件
            if let Ok(mut file) = fs::File::create(&segment_path).await {
                if let Err(e) = file.write_all(&ts_data).await {
                    error!(target: "hls_manager", "Failed to write segment: {}", e);
                } else {
                    info!(target: "hls_manager", 
                        stream_id = %context.stream_id,
                        filename = %segment_info.filename,
                        duration = duration,
                        size = total_size,
                        "HLS segment saved"
                    );
                }
            }
        }

        // 清空当前分片
        segment.data.clear();
        segment.duration = 0.0;

        Ok(())
    }

    /// 获取 M3U8 播放列表
    pub async fn get_playlist(&self, app_name: &str, stream_key: &str) -> Result<String> {
        let key = format!("{}/{}", app_name, stream_key);
        let generators = self.generators.read().await;

        if let Some(context) = generators.get(&key) {
            let hls_generator = context.hls_generator.read().await;
            hls_generator.generate_playlist().await
                .map_err(|e| anyhow::anyhow!("Failed to generate playlist: {}", e))
        } else {
            Err(anyhow::anyhow!("Stream not found: {}", key))
        }
    }

    /// 获取 TS 分片数据
    pub async fn get_segment(
        &self,
        app_name: &str,
        stream_key: &str,
        sequence: u64,
    ) -> Result<Bytes> {
        let key = format!("{}/{}", app_name, stream_key);
        let generators = self.generators.read().await;

        if let Some(context) = generators.get(&key) {
            let hls_generator = context.hls_generator.read().await;
            
            if let Some(segment_info) = hls_generator.get_segment(sequence).await {
                // 从磁盘读取 TS 数据
                let stream_dir = self.storage_dir.join(context.stream_id.as_str());
                let segment_path = stream_dir.join(&segment_info.filename);

                match fs::read(&segment_path).await {
                    Ok(data) => {
                        info!(target: "hls_manager", 
                            stream_key = %key,
                            sequence = sequence,
                            size = data.len(),
                            "Segment loaded from disk"
                        );
                        Ok(Bytes::from(data))
                    }
                    Err(e) => {
                        error!(target: "hls_manager", 
                            "Failed to read segment file: {}", e
                        );
                        Err(anyhow::anyhow!("Failed to read segment: {}", e))
                    }
                }
            } else {
                Err(anyhow::anyhow!("Segment not found: {}", sequence))
            }
        } else {
            Err(anyhow::anyhow!("Stream not found: {}", key))
        }
    }

    /// 处理音频数据
    pub async fn process_audio(
        &self,
        app_name: &str,
        stream_key: &str,
        data: &[u8],
        timestamp: u32,
    ) -> Result<()> {
        let key = format!("{}/{}", app_name, stream_key);
        let generators = self.generators.read().await;

        if let Some(_context) = generators.get(&key) {
            // TODO: 实现音频 TS 封装
            // 目前只处理视频，音频暂时忽略
            debug!(target: "hls_manager", 
                stream_key = %key,
                size = data.len(),
                timestamp = timestamp,
                "Audio data received (not yet processed)"
            );
        }

        Ok(())
    }

    /// 检查流是否存在
    pub async fn stream_exists(&self, app_name: &str, stream_key: &str) -> bool {
        let key = format!("{}/{}", app_name, stream_key);
        let generators = self.generators.read().await;
        generators.contains_key(&key)
    }
}

impl Default for HlsManager {
    fn default() -> Self {
        Self::new(PathBuf::from("./data/hls"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hls_manager_register() {
        use tempfile::tempdir;
        let temp_dir = tempdir().unwrap();
        let manager = HlsManager::new(temp_dir.path().to_path_buf());
        manager
            .register_stream("live".to_string(), "test".to_string(), 6)
            .await
            .unwrap();

        assert!(manager.stream_exists("live", "test").await);
    }

    #[tokio::test]
    async fn test_hls_manager_playlist() {
        use tempfile::tempdir;
        let temp_dir = tempdir().unwrap();
        let manager = HlsManager::new(temp_dir.path().to_path_buf());
        manager
            .register_stream("live".to_string(), "test".to_string(), 6)
            .await
            .unwrap();

        let playlist = manager.get_playlist("live", "test").await.unwrap();
        assert!(playlist.contains("#EXTM3U"));
        assert!(playlist.contains("#EXT-X-VERSION:3"));
    }

    #[tokio::test]
    async fn test_hls_manager_unregister() {
        use tempfile::tempdir;
        let temp_dir = tempdir().unwrap();
        let manager = HlsManager::new(temp_dir.path().to_path_buf());
        manager
            .register_stream("live".to_string(), "test".to_string(), 6)
            .await
            .unwrap();

        assert!(manager.stream_exists("live", "test").await);

        manager.unregister_stream("live", "test").await.unwrap();

        assert!(!manager.stream_exists("live", "test").await);
    }

    #[tokio::test]
    async fn test_hls_manager_process_video() {
        use tempfile::tempdir;
        let temp_dir = tempdir().unwrap();
        let manager = HlsManager::new(temp_dir.path().to_path_buf());
        manager
            .register_stream("live".to_string(), "test".to_string(), 6)
            .await
            .unwrap();

        // 模拟 H264 关键帧数据
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x67]; // SPS NALU

        manager
            .process_video("live", "test", &data, 1000, true)
            .await
            .unwrap();

        // 验证流仍然存在
        assert!(manager.stream_exists("live", "test").await);
    }
}
