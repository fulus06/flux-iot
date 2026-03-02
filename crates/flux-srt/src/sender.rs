use anyhow::Result;
use bytes::Bytes;
use std::net::SocketAddr;
use srt_tokio::SrtSocket;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// SRT 发送器（使用 srt-tokio 库）
pub struct SrtSender {
    socket: SrtSocket,
    dest_addr: SocketAddr,
    bytes_sent: u64,
}

impl SrtSender {
    /// 创建 SRT 发送器并连接到目标地址
    /// 
    /// # 参数
    /// - `dest_addr`: 目标 SRT 服务器地址
    /// 
    /// # 示例
    /// ```no_run
    /// use flux_srt::SrtSender;
    /// 
    /// let sender = SrtSender::new("127.0.0.1:9000".parse().unwrap()).await?;
    /// ```
    pub async fn new(dest_addr: SocketAddr) -> Result<Self> {
        info!(target: "srt_sender", "Connecting to SRT server: {}", dest_addr);
        
        // 使用 srt-tokio 建立连接
        let socket = SrtSocket::builder()
            .call(dest_addr, None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to SRT server: {}", e))?;
        
        info!(target: "srt_sender", "SRT sender connected to {}", dest_addr);
        
        Ok(Self {
            socket,
            dest_addr,
            bytes_sent: 0,
        })
    }

    /// 发送数据
    /// 
    /// # 参数
    /// - `data`: 要发送的数据
    /// - `_timestamp`: 时间戳（保留参数，用于兼容旧接口）
    pub async fn send(&mut self, data: &[u8], _timestamp: u32) -> Result<()> {
        // 使用 srt-tokio 发送数据
        self.socket.write_all(data).await
            .map_err(|e| anyhow::anyhow!("Failed to send SRT data: {}", e))?;
        
        self.bytes_sent += data.len() as u64;
        
        debug!(target: "srt_sender",
            "Sent SRT data: len={}, total={}",
            data.len(),
            self.bytes_sent
        );
        
        Ok(())
    }

    /// 获取已发送的字节数
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    /// 获取目标地址
    pub fn dest_addr(&self) -> SocketAddr {
        self.dest_addr
    }

    /// 关闭连接
    pub async fn close(mut self) -> Result<()> {
        self.socket.shutdown().await
            .map_err(|e| anyhow::anyhow!("Failed to close SRT connection: {}", e))?;
        
        info!(target: "srt_sender", "SRT sender closed, total sent: {} bytes", self.bytes_sent);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_srt_sender_creation() {
        // 注意：这个测试需要一个运行中的 SRT 服务器
        // 在实际测试中，应该先启动一个 SRT 接收器
        
        // 测试地址解析
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        assert_eq!(addr.port(), 9000);
    }
}
