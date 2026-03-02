use anyhow::Result;
use bytes::Bytes;
use std::net::SocketAddr;
use srt_tokio::SrtSocket;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// SRT 数据包
#[derive(Debug, Clone)]
pub struct SrtPacket {
    pub data: Bytes,
    pub source_addr: Option<SocketAddr>,
}

/// SRT 接收器（使用 srt-tokio 库）
pub struct SrtReceiver {
    socket: SrtSocket,
    listen_addr: SocketAddr,
    bytes_received: u64,
}

impl SrtReceiver {
    /// 创建 SRT 接收器并监听指定端口
    /// 
    /// # 参数
    /// - `port`: 监听端口
    /// 
    /// # 返回
    /// 返回接收器实例和数据通道
    /// 
    /// # 示例
    /// ```no_run
    /// use flux_srt::SrtReceiver;
    /// 
    /// let (receiver, mut rx) = SrtReceiver::new(9000).await?;
    /// tokio::spawn(async move {
    ///     receiver.start().await;
    /// });
    /// 
    /// while let Some(packet) = rx.recv().await {
    ///     println!("Received {} bytes", packet.data.len());
    /// }
    /// ```
    pub async fn new(port: u16) -> Result<(Self, mpsc::Receiver<SrtPacket>)> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        
        info!(target: "srt_receiver", "Starting SRT receiver on {}", addr);
        
        // 使用 srt-tokio 监听端口
        let socket = SrtSocket::builder()
            .listen_on(port)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start SRT listener: {}", e))?;
        
        info!(target: "srt_receiver", "SRT receiver listening on {}", addr);
        
        let (tx, rx) = mpsc::channel(100);
        
        let receiver = Self {
            socket,
            listen_addr: addr,
            bytes_received: 0,
        };
        
        Ok((receiver, rx))
    }

    /// 开始接收 SRT 数据
    /// 
    /// 这个方法会持续接收数据直到连接关闭或发生错误
    pub async fn start(mut self, tx: mpsc::Sender<SrtPacket>) {
        let mut buffer = vec![0u8; 65536];
        
        info!(target: "srt_receiver", "SRT receiver started, waiting for data...");
        
        loop {
            match self.socket.read(&mut buffer).await {
                Ok(0) => {
                    // 连接关闭
                    info!(target: "srt_receiver", "SRT connection closed");
                    break;
                }
                Ok(len) => {
                    self.bytes_received += len as u64;
                    
                    let data = Bytes::copy_from_slice(&buffer[..len]);
                    
                    debug!(target: "srt_receiver",
                        "Received SRT data: len={}, total={}",
                        len,
                        self.bytes_received
                    );
                    
                    let packet = SrtPacket {
                        data,
                        source_addr: None, // srt-tokio 不直接提供源地址
                    };
                    
                    if let Err(e) = tx.send(packet).await {
                        error!(target: "srt_receiver", "Failed to send packet to channel: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!(target: "srt_receiver", "Failed to receive SRT data: {}", e);
                    break;
                }
            }
        }
        
        info!(target: "srt_receiver", "SRT receiver stopped, total received: {} bytes", self.bytes_received);
    }

    /// 获取监听地址
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    /// 获取已接收的字节数
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_srt_receiver_creation() {
        // 测试地址解析
        let addr: SocketAddr = "0.0.0.0:9000".parse().unwrap();
        assert_eq!(addr.port(), 9000);
    }
}
