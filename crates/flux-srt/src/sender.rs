use anyhow::Result;
use bytes::Bytes;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::{debug, info};

/// SRT 发送器（简化实现）
pub struct SrtSender {
    socket: UdpSocket,
    dest_addr: SocketAddr,
    sequence: u32,
}

impl SrtSender {
    /// 创建 SRT 发送器
    pub async fn new(dest_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        
        info!(target: "srt_sender", "SRT sender created, target: {}", dest_addr);
        
        Ok(Self {
            socket,
            dest_addr,
            sequence: 0,
        })
    }

    /// 发送数据
    pub async fn send(&mut self, data: &[u8], timestamp: u32) -> Result<()> {
        let packet = self.build_data_packet(data, timestamp);
        
        self.socket.send_to(&packet, self.dest_addr).await?;
        
        debug!(target: "srt_sender",
            "Sent SRT packet: seq={}, ts={}, len={}",
            self.sequence,
            timestamp,
            data.len()
        );
        
        self.sequence = self.sequence.wrapping_add(1);
        
        Ok(())
    }

    /// 构建 SRT 数据包
    fn build_data_packet(&self, data: &[u8], timestamp: u32) -> Vec<u8> {
        let mut packet = Vec::with_capacity(16 + data.len());
        
        // Flags (data packet, no encryption)
        packet.extend_from_slice(&0u32.to_be_bytes());
        
        // Timestamp
        packet.extend_from_slice(&timestamp.to_be_bytes());
        
        // Destination Socket ID (0 for now)
        packet.extend_from_slice(&0u32.to_be_bytes());
        
        // Sequence number
        packet.extend_from_slice(&self.sequence.to_be_bytes());
        
        // Payload
        packet.extend_from_slice(data);
        
        packet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_data_packet() {
        // 创建真实的 sender 来测试
        let sender_result = SrtSender::new("127.0.0.1:9000".parse().unwrap()).await;
        assert!(sender_result.is_ok());
        
        if let Ok(mut sender) = sender_result {
            sender.sequence = 42; // 设置测试序列号
            let packet = sender.build_data_packet(b"test", 100);
            
            assert_eq!(packet.len(), 16 + 4);
            assert_eq!(&packet[16..], b"test");
            
            // Check timestamp
            let ts = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
            assert_eq!(ts, 100);
            
            // Check sequence
            let seq = u32::from_be_bytes([packet[12], packet[13], packet[14], packet[15]]);
            assert_eq!(seq, 42);
        }
    }
}
