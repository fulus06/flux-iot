// RTP 接收器
// 监听 UDP 端口，接收和缓冲 RTP 数据包

use super::packet::RtpPacket;
use crate::Result;
use bytes::Bytes;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

/// RTP 接收器配置
#[derive(Debug, Clone)]
pub struct RtpReceiverConfig {
    /// 监听地址
    pub bind_addr: String,
    
    /// 接收缓冲区大小
    pub buffer_size: usize,
    
    /// 通道缓冲区大小
    pub channel_buffer: usize,
}

impl Default for RtpReceiverConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:9000".to_string(),
            buffer_size: 65536,
            channel_buffer: 1000,
        }
    }
}

/// RTP 流信息
#[derive(Debug)]
pub struct RtpStream {
    /// SSRC
    pub ssrc: u32,
    
    /// 数据包发送器
    pub sender: mpsc::Sender<RtpPacket>,
    
    /// 最后接收时间
    pub last_received: std::time::Instant,
}

/// RTP 接收器
pub struct RtpReceiver {
    config: RtpReceiverConfig,
    socket: Arc<UdpSocket>,
    streams: Arc<tokio::sync::RwLock<HashMap<u32, RtpStream>>>,
}

impl RtpReceiver {
    /// 创建 RTP 接收器
    pub async fn new(config: RtpReceiverConfig) -> Result<Self> {
        let socket = UdpSocket::bind(&config.bind_addr).await
            .map_err(|e| crate::VideoError::Other(format!("Failed to bind UDP socket: {}", e)))?;
        
        tracing::info!("RTP receiver listening on {}", config.bind_addr);
        
        Ok(Self {
            config,
            socket: Arc::new(socket),
            streams: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        })
    }
    
    /// 启动接收器
    pub async fn start(self: Arc<Self>) -> Result<()> {
        tracing::info!("RTP receiver started");
        
        let mut buf = vec![0u8; self.config.buffer_size];
        
        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = Bytes::copy_from_slice(&buf[..len]);
                    let receiver = self.clone();
                    
                    // 异步处理数据包
                    tokio::spawn(async move {
                        if let Err(e) = receiver.handle_packet(data, addr).await {
                            tracing::error!("Failed to handle RTP packet from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to receive UDP packet: {}", e);
                }
            }
        }
    }
    
    /// 处理 RTP 数据包
    async fn handle_packet(&self, data: Bytes, _addr: SocketAddr) -> Result<()> {
        // 解析 RTP 数据包
        let packet = RtpPacket::from_bytes(data)
            .ok_or_else(|| crate::VideoError::Other("Failed to parse RTP packet".to_string()))?;
        
        let ssrc = packet.header.ssrc;
        
        // 获取或创建流
        let streams = self.streams.read().await;
        
        if let Some(stream) = streams.get(&ssrc) {
            // 发送数据包到流
            if let Err(e) = stream.sender.try_send(packet) {
                tracing::warn!("Failed to send RTP packet to stream {}: {}", ssrc, e);
            }
        } else {
            drop(streams);
            tracing::debug!("Received RTP packet from unknown SSRC: {}", ssrc);
        }
        
        Ok(())
    }
    
    /// 注册 RTP 流
    pub async fn register_stream(&self, ssrc: u32) -> mpsc::Receiver<RtpPacket> {
        let (tx, rx) = mpsc::channel(self.config.channel_buffer);
        
        let stream = RtpStream {
            ssrc,
            sender: tx,
            last_received: std::time::Instant::now(),
        };
        
        let mut streams = self.streams.write().await;
        streams.insert(ssrc, stream);
        
        tracing::info!("Registered RTP stream: SSRC={}", ssrc);
        
        rx
    }
    
    /// 注销 RTP 流
    pub async fn unregister_stream(&self, ssrc: u32) {
        let mut streams = self.streams.write().await;
        streams.remove(&ssrc);
        
        tracing::info!("Unregistered RTP stream: SSRC={}", ssrc);
    }
    
    /// 获取活跃流数量
    pub async fn active_streams(&self) -> usize {
        let streams = self.streams.read().await;
        streams.len()
    }
    
    /// 清理超时流
    pub async fn cleanup_timeout(&self, timeout_secs: u64) -> usize {
        let mut streams = self.streams.write().await;
        let now = std::time::Instant::now();
        
        let timeout_ssrcs: Vec<u32> = streams
            .iter()
            .filter(|(_, stream)| {
                now.duration_since(stream.last_received).as_secs() > timeout_secs
            })
            .map(|(ssrc, _)| *ssrc)
            .collect();
        
        let count = timeout_ssrcs.len();
        for ssrc in timeout_ssrcs {
            streams.remove(&ssrc);
            tracing::info!("Removed timeout RTP stream: SSRC={}", ssrc);
        }
        
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_rtp_receiver_creation() {
        let config = RtpReceiverConfig {
            bind_addr: "127.0.0.1:0".to_string(), // 随机端口
            ..Default::default()
        };
        
        let receiver = RtpReceiver::new(config).await;
        assert!(receiver.is_ok());
    }
    
    #[tokio::test]
    async fn test_stream_registration() {
        let config = RtpReceiverConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            ..Default::default()
        };
        
        let receiver = RtpReceiver::new(config).await.unwrap();
        
        let ssrc = 0x12345678;
        let mut rx = receiver.register_stream(ssrc).await;
        
        assert_eq!(receiver.active_streams().await, 1);
        
        // 注销流
        receiver.unregister_stream(ssrc).await;
        assert_eq!(receiver.active_streams().await, 0);
    }
}
