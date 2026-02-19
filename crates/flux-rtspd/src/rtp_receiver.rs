use anyhow::Result;
use bytes::{Buf, Bytes};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// RTP 包
#[derive(Debug, Clone)]
pub struct RtpPacket {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub csrc_count: u8,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: Bytes,
}

/// RTP 接收器
pub struct RtpReceiver {
    socket: UdpSocket,
    tx: mpsc::Sender<RtpPacket>,
}

impl RtpReceiver {
    /// 创建 RTP 接收器
    pub async fn new(port: u16) -> Result<(Self, mpsc::Receiver<RtpPacket>)> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let socket = UdpSocket::bind(addr).await?;
        
        info!(target: "rtp_receiver", "RTP receiver listening on {}", addr);
        
        let (tx, rx) = mpsc::channel(100);
        
        Ok((Self { socket, tx }, rx))
    }

    /// 开始接收 RTP 包
    pub async fn start(mut self) {
        let mut buffer = vec![0u8; 2048];
        
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, _addr)) => {
                    if len < 12 {
                        continue; // RTP 头部至少 12 字节
                    }
                    
                    match Self::parse_rtp_packet(&buffer[..len]) {
                        Ok(packet) => {
                            debug!(target: "rtp_receiver",
                                "RTP packet: seq={}, ts={}, pt={}, len={}",
                                packet.sequence_number,
                                packet.timestamp,
                                packet.payload_type,
                                packet.payload.len()
                            );
                            
                            if let Err(e) = self.tx.send(packet).await {
                                error!(target: "rtp_receiver", "Failed to send RTP packet: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!(target: "rtp_receiver", "Failed to parse RTP packet: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!(target: "rtp_receiver", "Failed to receive UDP packet: {}", e);
                    break;
                }
            }
        }
    }

    /// 解析 RTP 包
    fn parse_rtp_packet(data: &[u8]) -> Result<RtpPacket> {
        if data.len() < 12 {
            return Err(anyhow::anyhow!("RTP packet too short"));
        }
        
        let mut cursor = std::io::Cursor::new(data);
        
        // Byte 0: V(2), P(1), X(1), CC(4)
        let byte0 = cursor.get_u8();
        let version = (byte0 >> 6) & 0x03;
        let padding = (byte0 & 0x20) != 0;
        let extension = (byte0 & 0x10) != 0;
        let csrc_count = byte0 & 0x0F;
        
        // Byte 1: M(1), PT(7)
        let byte1 = cursor.get_u8();
        let marker = (byte1 & 0x80) != 0;
        let payload_type = byte1 & 0x7F;
        
        // Bytes 2-3: Sequence number
        let sequence_number = cursor.get_u16();
        
        // Bytes 4-7: Timestamp
        let timestamp = cursor.get_u32();
        
        // Bytes 8-11: SSRC
        let ssrc = cursor.get_u32();
        
        // Skip CSRC identifiers
        let csrc_size = csrc_count as usize * 4;
        if data.len() < 12 + csrc_size {
            return Err(anyhow::anyhow!("Invalid CSRC count"));
        }
        cursor.set_position(12 + csrc_size as u64);
        
        // Skip extension if present
        if extension {
            if cursor.remaining() < 4 {
                return Err(anyhow::anyhow!("Invalid extension"));
            }
            let _ext_profile = cursor.get_u16();
            let ext_length = cursor.get_u16() as usize * 4;
            if cursor.remaining() < ext_length {
                return Err(anyhow::anyhow!("Invalid extension length"));
            }
            cursor.set_position(cursor.position() + ext_length as u64);
        }
        
        // Payload
        let payload_start = cursor.position() as usize;
        let mut payload_end = data.len();
        
        // Remove padding if present
        if padding && !data.is_empty() {
            let padding_len = data[data.len() - 1] as usize;
            if padding_len > 0 && padding_len <= data.len() - payload_start {
                payload_end -= padding_len;
            }
        }
        
        let payload = Bytes::copy_from_slice(&data[payload_start..payload_end]);
        
        Ok(RtpPacket {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rtp_packet() {
        // 构造一个简单的 RTP 包
        let mut data = vec![
            0x80, // V=2, P=0, X=0, CC=0
            0x60, // M=0, PT=96
            0x00, 0x01, // Sequence = 1
            0x00, 0x00, 0x00, 0x64, // Timestamp = 100
            0x12, 0x34, 0x56, 0x78, // SSRC
        ];
        data.extend_from_slice(b"payload");
        
        let packet = RtpReceiver::parse_rtp_packet(&data).unwrap();
        
        assert_eq!(packet.version, 2);
        assert_eq!(packet.payload_type, 96);
        assert_eq!(packet.sequence_number, 1);
        assert_eq!(packet.timestamp, 100);
        assert_eq!(packet.ssrc, 0x12345678);
        assert_eq!(packet.payload.as_ref(), b"payload");
    }

    #[test]
    fn test_parse_rtp_packet_with_marker() {
        let mut data = vec![
            0x80, // V=2, P=0, X=0, CC=0
            0xE0, // M=1, PT=96
            0x00, 0x02, // Sequence = 2
            0x00, 0x00, 0x00, 0xC8, // Timestamp = 200
            0xAB, 0xCD, 0xEF, 0x00, // SSRC
        ];
        data.extend_from_slice(b"test");
        
        let packet = RtpReceiver::parse_rtp_packet(&data).unwrap();
        
        assert!(packet.marker);
        assert_eq!(packet.payload_type, 96);
    }
}
