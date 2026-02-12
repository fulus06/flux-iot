// GB28181 视频流处理
// 集成 RTP 接收、PS 解封装和存储

use super::rtp::{RtpReceiver, RtpPacket};
use super::ps::PsDemuxer;
use crate::storage::StandaloneStorage;
use crate::Result;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::mpsc;

/// GB28181 流配置
#[derive(Debug, Clone)]
pub struct Gb28181StreamConfig {
    /// 流 ID
    pub stream_id: String,
    
    /// SSRC
    pub ssrc: u32,
    
    /// RTP 端口
    pub rtp_port: u16,
}

/// GB28181 视频流
pub struct Gb28181Stream {
    config: Gb28181StreamConfig,
    rtp_receiver: Arc<RtpReceiver>,
    storage: Arc<tokio::sync::RwLock<StandaloneStorage>>,
    running: Arc<tokio::sync::RwLock<bool>>,
}

impl Gb28181Stream {
    /// 创建 GB28181 流
    pub fn new(
        config: Gb28181StreamConfig,
        rtp_receiver: Arc<RtpReceiver>,
        storage: Arc<tokio::sync::RwLock<StandaloneStorage>>,
    ) -> Self {
        Self {
            config,
            rtp_receiver,
            storage,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }
    
    /// 启动流处理
    pub async fn start(self: Arc<Self>) -> Result<()> {
        // 设置运行状态
        {
            let mut running = self.running.write().await;
            *running = true;
        }
        
        // 注册 RTP 流
        let mut rtp_rx = self.rtp_receiver.register_stream(self.config.ssrc).await;
        
        tracing::info!(
            "GB28181 stream started: {} (SSRC={})",
            self.config.stream_id,
            self.config.ssrc
        );
        
        // 创建处理任务
        let stream = self.clone();
        tokio::spawn(async move {
            stream.process_loop(&mut rtp_rx).await;
        });
        
        Ok(())
    }
    
    /// 处理循环
    async fn process_loop(&self, rtp_rx: &mut mpsc::Receiver<RtpPacket>) {
        let mut ps_demuxer = PsDemuxer::new();
        let mut frame_buffer = Vec::new();
        
        while *self.running.read().await {
            // 接收 RTP 包
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(5),
                rtp_rx.recv()
            ).await {
                Ok(Some(rtp_packet)) => {
                    // 输入到 PS 解封装器
                    ps_demuxer.input(rtp_packet.payload().clone());
                    
                    // 提取视频数据
                    while let Some(video_data) = ps_demuxer.pop_video() {
                        frame_buffer.extend_from_slice(&video_data);
                        
                        // 如果是标记包（帧结束），保存数据
                        if rtp_packet.is_marker() && !frame_buffer.is_empty() {
                            if let Err(e) = self.save_frame(&frame_buffer).await {
                                tracing::error!("Failed to save frame: {}", e);
                            }
                            frame_buffer.clear();
                        }
                    }
                }
                Ok(None) => {
                    // 通道关闭
                    tracing::info!("RTP channel closed for stream {}", self.config.stream_id);
                    break;
                }
                Err(_) => {
                    // 超时
                    tracing::warn!("RTP receive timeout for stream {}", self.config.stream_id);
                }
            }
        }
        
        tracing::info!("GB28181 stream stopped: {}", self.config.stream_id);
    }
    
    /// 保存帧数据
    async fn save_frame(&self, data: &[u8]) -> Result<()> {
        let timestamp = chrono::Utc::now();
        let bytes = Bytes::copy_from_slice(data);
        
        let mut storage = self.storage.write().await;
        storage.put_object(&self.config.stream_id, timestamp, bytes).await?;
        
        tracing::debug!(
            "Saved frame for stream {}: {} bytes",
            self.config.stream_id,
            data.len()
        );
        
        Ok(())
    }
    
    /// 停止流处理
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        
        // 注销 RTP 流
        self.rtp_receiver.unregister_stream(self.config.ssrc).await;
        
        tracing::info!("GB28181 stream stopping: {}", self.config.stream_id);
    }
    
    /// 获取流 ID
    pub fn stream_id(&self) -> &str {
        &self.config.stream_id
    }
    
    /// 获取 SSRC
    pub fn ssrc(&self) -> u32 {
        self.config.ssrc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb28181::rtp::receiver::RtpReceiverConfig;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_gb28181_stream_creation() {
        let temp_dir = TempDir::new().unwrap();
        
        let rtp_config = RtpReceiverConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            ..Default::default()
        };
        
        let rtp_receiver = Arc::new(RtpReceiver::new(rtp_config).await.unwrap());
        let storage = Arc::new(tokio::sync::RwLock::new(
            StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap()
        ));
        
        let config = Gb28181StreamConfig {
            stream_id: "test_stream".to_string(),
            ssrc: 0x12345678,
            rtp_port: 9000,
        };
        
        let stream = Gb28181Stream::new(config, rtp_receiver, storage);
        
        assert_eq!(stream.stream_id(), "test_stream");
        assert_eq!(stream.ssrc(), 0x12345678);
    }
}
