use anyhow::Result;
use bytes::Bytes;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// SRT 数据包
#[derive(Debug, Clone)]
pub struct SrtPacket {
    pub timestamp: u32,
    pub data: Bytes,
    pub is_control: bool,
}

/// SRT 接收器（简化实现）
pub struct SrtReceiver {
    socket: UdpSocket,
    tx: mpsc::Sender<SrtPacket>,
}

impl SrtReceiver {
    /// 创建 SRT 接收器
    pub async fn new(port: u16) -> Result<(Self, mpsc::Receiver<SrtPacket>)> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let socket = UdpSocket::bind(addr).await?;
        
        info!(target: "srt_receiver", "SRT receiver listening on {}", addr);
        
        let (tx, rx) = mpsc::channel(100);
        
        Ok((Self { socket, tx }, rx))
    }

    /// 开始接收 SRT 包
    pub async fn start(mut self) {
        let mut buffer = vec![0u8; 65536];
        
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, addr)) => {
                    if len < 16 {
                        continue; // SRT 头部至少 16 字节
                    }
                    
                    match Self::parse_srt_packet(&buffer[..len]) {
                        Ok(packet) => {
                            debug!(target: "srt_receiver",
                                "SRT packet from {}: ts={}, len={}, control={}",
                                addr,
                                packet.timestamp,
                                packet.data.len(),
                                packet.is_control
                            );
                            
                            if let Err(e) = self.tx.send(packet).await {
                                error!(target: "srt_receiver", "Failed to send SRT packet: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!(target: "srt_receiver", "Failed to parse SRT packet: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!(target: "srt_receiver", "Failed to receive UDP packet: {}", e);
                    break;
                }
            }
        }
    }

    /// 解析 SRT 包（简化版本）
    fn parse_srt_packet(data: &[u8]) -> Result<SrtPacket> {
        if data.len() < 16 {
            return Err(anyhow::anyhow!("SRT packet too short"));
        }
        
        // SRT 包格式（简化）:
        // Byte 0-3: Flags + Type
        // Byte 4-7: Timestamp
        // Byte 8-11: Destination Socket ID
        // Byte 12-15: Sequence Number
        // Byte 16+: Payload
        
        let flags = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let is_control = (flags & 0x80000000) != 0;
        
        let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        
        let payload = if data.len() > 16 {
            Bytes::copy_from_slice(&data[16..])
        } else {
            Bytes::new()
        };
        
        Ok(SrtPacket {
            timestamp,
            data: payload,
            is_control,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_srt_packet() {
        // 构造一个简单的 SRT 数据包
        let mut data = vec![
            0x00, 0x00, 0x00, 0x00, // Flags (data packet)
            0x00, 0x00, 0x00, 0x64, // Timestamp = 100
            0x12, 0x34, 0x56, 0x78, // Dest Socket ID
            0x00, 0x00, 0x00, 0x01, // Sequence = 1
        ];
        data.extend_from_slice(b"payload");
        
        let packet = SrtReceiver::parse_srt_packet(&data).unwrap();
        
        assert!(!packet.is_control);
        assert_eq!(packet.timestamp, 100);
        assert_eq!(packet.data.as_ref(), b"payload");
    }

    #[test]
    fn test_parse_srt_control_packet() {
        let data = vec![
            0x80, 0x00, 0x00, 0x00, // Flags (control packet)
            0x00, 0x00, 0x00, 0xC8, // Timestamp = 200
            0xAB, 0xCD, 0xEF, 0x00, // Dest Socket ID
            0x00, 0x00, 0x00, 0x02, // Sequence = 2
        ];
        
        let packet = SrtReceiver::parse_srt_packet(&data).unwrap();
        
        assert!(packet.is_control);
        assert_eq!(packet.timestamp, 200);
    }
}
