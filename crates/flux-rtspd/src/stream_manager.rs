use anyhow::{anyhow, Result};
use flux_media_core::{
    snapshot::SnapshotOrchestrator,
    storage::filesystem::FileSystemStorage,
    types::StreamId,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

use crate::h264_depacketizer::{H264Depacketizer, H264Nalu};
use crate::rtp_receiver::RtpReceiver;
use crate::rtsp_client::RtspClient;
use crate::sdp_parser::SdpParser;

/// RTSP 流信息
#[derive(Debug, Clone)]
pub struct RtspStreamInfo {
    pub stream_id: StreamId,
    pub url: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub is_active: bool,
    pub frame_count: u64,
    pub last_keyframe_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// RTSP 流管理器
pub struct RtspStreamManager {
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
    streams: Arc<RwLock<std::collections::HashMap<String, RtspStreamInfo>>>,
}

impl RtspStreamManager {
    pub fn new(
        storage: Arc<RwLock<FileSystemStorage>>,
        orchestrator: Arc<SnapshotOrchestrator>,
    ) -> Self {
        Self {
            storage,
            orchestrator,
            streams: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 启动 RTSP 流
    pub async fn start_stream(&self, url: String) -> Result<()> {
        let stream_id = Self::url_to_stream_id(&url);
        
        // 检查是否已存在
        {
            let streams = self.streams.read().await;
            if streams.contains_key(&url) {
                return Err(anyhow!("Stream already exists: {}", url));
            }
        }
        
        // 注册流信息
        {
            let mut streams = self.streams.write().await;
            streams.insert(url.clone(), RtspStreamInfo {
                stream_id: stream_id.clone(),
                url: url.clone(),
                start_time: chrono::Utc::now(),
                is_active: true,
                frame_count: 0,
                last_keyframe_time: None,
            });
        }
        
        info!(target: "rtsp_stream_manager", "Starting RTSP stream: {}", url);
        
        // 启动流处理任务
        let storage = self.storage.clone();
        let orchestrator = self.orchestrator.clone();
        let streams = self.streams.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::stream_task(
                url.clone(),
                stream_id,
                storage,
                orchestrator,
                streams,
            ).await {
                error!(target: "rtsp_stream_manager", "Stream task failed: {}", e);
            }
        });
        
        Ok(())
    }

    /// 停止 RTSP 流
    pub async fn stop_stream(&self, url: &str) -> Result<()> {
        let mut streams = self.streams.write().await;
        if let Some(info) = streams.get_mut(url) {
            info.is_active = false;
            info!(target: "rtsp_stream_manager", "Stopping RTSP stream: {}", url);
            Ok(())
        } else {
            Err(anyhow!("Stream not found: {}", url))
        }
    }

    /// 获取流信息
    pub async fn get_stream_info(&self, url: &str) -> Option<RtspStreamInfo> {
        let streams = self.streams.read().await;
        streams.get(url).cloned()
    }

    /// 列出所有流
    pub async fn list_streams(&self) -> Vec<RtspStreamInfo> {
        let streams = self.streams.read().await;
        streams.values().cloned().collect()
    }

    /// 流处理任务
    async fn stream_task(
        url: String,
        stream_id: StreamId,
        storage: Arc<RwLock<FileSystemStorage>>,
        orchestrator: Arc<SnapshotOrchestrator>,
        streams: Arc<RwLock<std::collections::HashMap<String, RtspStreamInfo>>>,
    ) -> Result<()> {
        loop {
            // 检查流是否仍然活跃
            {
                let streams_lock = streams.read().await;
                if let Some(info) = streams_lock.get(&url) {
                    if !info.is_active {
                        info!(target: "rtsp_stream_manager", "Stream stopped: {}", url);
                        break;
                    }
                } else {
                    break;
                }
            }
            
            // 尝试连接和处理流
            match Self::process_stream(
                &url,
                &stream_id,
                &storage,
                &orchestrator,
                &streams,
            ).await {
                Ok(_) => {
                    info!(target: "rtsp_stream_manager", "Stream ended normally: {}", url);
                    break;
                }
                Err(e) => {
                    error!(target: "rtsp_stream_manager", "Stream error: {}, retrying in 5s", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
        
        // 清理流信息
        let mut streams_lock = streams.write().await;
        streams_lock.remove(&url);
        
        Ok(())
    }

    /// 处理流
    async fn process_stream(
        url: &str,
        stream_id: &StreamId,
        storage: &Arc<RwLock<FileSystemStorage>>,
        orchestrator: &Arc<SnapshotOrchestrator>,
        streams: &Arc<RwLock<std::collections::HashMap<String, RtspStreamInfo>>>,
    ) -> Result<()> {
        // 1. 创建 RTSP 客户端
        let mut client = RtspClient::new(url.to_string());
        client.connect().await?;
        
        // 2. OPTIONS
        client.options().await?;
        
        // 3. DESCRIBE (获取 SDP)
        let describe_resp = client.describe().await?;
        let sdp = SdpParser::parse(&describe_resp.body)?;
        
        // 4. 获取视频轨道
        let video_track = SdpParser::get_video_track(&sdp)
            .ok_or_else(|| anyhow!("No video track found"))?;
        
        // 5. SETUP (使用随机端口)
        let rtp_port = 5000 + (rand::random::<u16>() % 1000);
        let track_url = if let Some(control) = &video_track.control_url {
            if control.starts_with("rtsp://") {
                control.clone()
            } else {
                format!("{}/{}", url.trim_end_matches('/'), control)
            }
        } else {
            url.to_string()
        };
        
        client.setup(&track_url, rtp_port).await?;
        
        // 6. 启动 RTP 接收器
        let (receiver, mut rtp_rx) = RtpReceiver::new(rtp_port).await?;
        tokio::spawn(async move {
            receiver.start().await;
        });
        
        // 7. PLAY
        client.play().await?;
        
        info!(target: "rtsp_stream_manager", "RTSP stream playing: {}", url);
        
        // 8. 处理 RTP 数据
        let mut depacketizer = H264Depacketizer::new();
        let mut frame_count = 0u64;
        
        while let Some(rtp_packet) = rtp_rx.recv().await {
            // 检查流是否仍然活跃
            {
                let streams_lock = streams.read().await;
                if let Some(info) = streams_lock.get(url) {
                    if !info.is_active {
                        break;
                    }
                }
            }
            
            // 解包 H264
            match depacketizer.process_rtp(rtp_packet) {
                Ok(nalus) => {
                    for nalu in nalus {
                        frame_count += 1;
                        
                        // 保存到存储
                        Self::save_nalu(
                            stream_id,
                            &nalu,
                            storage,
                            orchestrator,
                        ).await?;
                        
                        // 更新流信息
                        if nalu.is_keyframe {
                            let mut streams_lock = streams.write().await;
                            if let Some(info) = streams_lock.get_mut(url) {
                                info.frame_count = frame_count;
                                info.last_keyframe_time = Some(chrono::Utc::now());
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(target: "rtsp_stream_manager", "Failed to depacketize RTP: {}", e);
                }
            }
        }
        
        // 9. TEARDOWN
        let _ = client.teardown().await;
        
        Ok(())
    }

    /// 保存 NALU 到存储
    async fn save_nalu(
        stream_id: &StreamId,
        nalu: &H264Nalu,
        _storage: &Arc<RwLock<FileSystemStorage>>,
        orchestrator: &Arc<SnapshotOrchestrator>,
    ) -> Result<()> {
        // 如果是关键帧，提取 snapshot
        if nalu.is_keyframe {
            debug!(target: "rtsp_stream_manager", 
                "Keyframe detected for {}, size={}", 
                stream_id.as_str(), 
                nalu.data.len()
            );
            
            // 处理关键帧用于 snapshot
            let timestamp = chrono::Utc::now();
            if let Err(e) = orchestrator.process_keyframe(stream_id, &nalu.data, timestamp).await {
                warn!(target: "rtsp_stream_manager", "Failed to process keyframe: {}", e);
            }
        }
        
        Ok(())
    }

    /// URL 转换为 StreamId
    fn url_to_stream_id(url: &str) -> StreamId {
        // rtsp://192.168.1.100:554/stream1 -> rtsp/192.168.1.100:554/stream1
        let path = url.strip_prefix("rtsp://").unwrap_or(url);
        StreamId::new("rtsp", path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_to_stream_id() {
        let stream_id = RtspStreamManager::url_to_stream_id("rtsp://192.168.1.100:554/stream1");
        assert_eq!(stream_id.as_str(), "rtsp/192.168.1.100:554/stream1");
    }

    #[test]
    fn test_stream_info_creation() {
        let info = RtspStreamInfo {
            stream_id: StreamId::new("rtsp", "192.168.1.100:554/stream1"),
            url: "rtsp://192.168.1.100:554/stream1".to_string(),
            start_time: chrono::Utc::now(),
            is_active: true,
            frame_count: 0,
            last_keyframe_time: None,
        };
        
        assert!(info.is_active);
        assert_eq!(info.frame_count, 0);
    }
}
