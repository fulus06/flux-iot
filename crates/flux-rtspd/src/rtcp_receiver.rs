use anyhow::Result;
use bytes::Buf;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// RTCP 包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtcpPacketType {
    SR = 200,   // Sender Report
    RR = 201,   // Receiver Report
    SDES = 202, // Source Description
    BYE = 203,  // Goodbye
    APP = 204,  // Application-defined
}

impl RtcpPacketType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            200 => Some(Self::SR),
            201 => Some(Self::RR),
            202 => Some(Self::SDES),
            203 => Some(Self::BYE),
            204 => Some(Self::APP),
            _ => None,
        }
    }
}

/// RTCP Sender Report
#[derive(Debug, Clone)]
pub struct SenderReport {
    pub ssrc: u32,
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub packet_count: u32,
    pub octet_count: u32,
}

/// RTCP Receiver Report
#[derive(Debug, Clone)]
pub struct ReceiverReport {
    pub ssrc: u32,
    pub report_blocks: Vec<ReportBlock>,
}

/// RTCP Report Block
#[derive(Debug, Clone)]
pub struct ReportBlock {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub cumulative_lost: u32,
    pub highest_seq: u32,
    pub jitter: u32,
    pub lsr: u32,  // Last SR timestamp
    pub dlsr: u32, // Delay since last SR
}

/// RTCP 包
#[derive(Debug, Clone)]
pub enum RtcpPacket {
    SenderReport(SenderReport),
    ReceiverReport(ReceiverReport),
    Unknown(u8),
}

/// RTCP 接收器
pub struct RtcpReceiver {
    socket: UdpSocket,
    tx: mpsc::Sender<RtcpPacket>,
}

impl RtcpReceiver {
    /// 创建 RTCP 接收器
    pub async fn new(port: u16) -> Result<(Self, mpsc::Receiver<RtcpPacket>)> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let socket = UdpSocket::bind(addr).await?;
        
        info!(target: "rtcp_receiver", "RTCP receiver listening on {}", addr);
        
        let (tx, rx) = mpsc::channel(100);
        
        Ok((Self { socket, tx }, rx))
    }

    /// 开始接收 RTCP 包
    pub async fn start(mut self) {
        let mut buffer = vec![0u8; 2048];
        
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, _addr)) => {
                    if len < 8 {
                        continue; // RTCP 头部至少 8 字节
                    }
                    
                    match Self::parse_rtcp_packet(&buffer[..len]) {
                        Ok(packets) => {
                            for packet in packets {
                                debug!(target: "rtcp_receiver", "RTCP packet: {:?}", packet);
                                
                                if let Err(e) = self.tx.send(packet).await {
                                    error!(target: "rtcp_receiver", "Failed to send RTCP packet: {}", e);
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            error!(target: "rtcp_receiver", "Failed to parse RTCP packet: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!(target: "rtcp_receiver", "Failed to receive UDP packet: {}", e);
                    break;
                }
            }
        }
    }

    /// 解析 RTCP 包（可能包含多个复合包）
    pub fn parse_rtcp_packet(data: &[u8]) -> Result<Vec<RtcpPacket>> {
        let mut packets = Vec::new();
        let mut cursor = std::io::Cursor::new(data);
        
        while cursor.remaining() >= 8 {
            let start_pos = cursor.position() as usize;
            
            // Byte 0: V(2), P(1), RC/SC(5)
            let byte0 = cursor.get_u8();
            let version = (byte0 >> 6) & 0x03;
            let padding = (byte0 & 0x20) != 0;
            let count = byte0 & 0x1F;
            
            if version != 2 {
                break; // 不是 RTP/RTCP v2
            }
            
            // Byte 1: Packet Type
            let packet_type = cursor.get_u8();
            
            // Bytes 2-3: Length (in 32-bit words, minus 1)
            let length = cursor.get_u16() as usize;
            let packet_len = (length + 1) * 4;
            
            if cursor.remaining() + 4 < packet_len {
                break; // 包不完整
            }
            
            // 解析具体类型
            let packet = match RtcpPacketType::from_u8(packet_type) {
                Some(RtcpPacketType::SR) => {
                    Self::parse_sender_report(&data[start_pos..start_pos + packet_len])?
                }
                Some(RtcpPacketType::RR) => {
                    Self::parse_receiver_report(&data[start_pos..start_pos + packet_len], count)?
                }
                _ => RtcpPacket::Unknown(packet_type),
            };
            
            packets.push(packet);
            
            // 跳过 padding
            if padding && cursor.remaining() > 0 {
                let padding_len = data[start_pos + packet_len - 1] as usize;
                cursor.set_position((start_pos + packet_len - padding_len) as u64);
            } else {
                cursor.set_position((start_pos + packet_len) as u64);
            }
        }
        
        Ok(packets)
    }

    /// 解析 Sender Report
    fn parse_sender_report(data: &[u8]) -> Result<RtcpPacket> {
        if data.len() < 28 {
            return Err(anyhow::anyhow!("SR packet too short"));
        }
        
        let mut cursor = std::io::Cursor::new(&data[4..]); // Skip header
        
        let ssrc = cursor.get_u32();
        let ntp_timestamp = cursor.get_u64();
        let rtp_timestamp = cursor.get_u32();
        let packet_count = cursor.get_u32();
        let octet_count = cursor.get_u32();
        
        Ok(RtcpPacket::SenderReport(SenderReport {
            ssrc,
            ntp_timestamp,
            rtp_timestamp,
            packet_count,
            octet_count,
        }))
    }

    /// 解析 Receiver Report
    fn parse_receiver_report(data: &[u8], count: u8) -> Result<RtcpPacket> {
        if data.len() < 8 {
            return Err(anyhow::anyhow!("RR packet too short"));
        }
        
        let mut cursor = std::io::Cursor::new(&data[4..]); // Skip header
        let ssrc = cursor.get_u32();
        
        let mut report_blocks = Vec::new();
        
        for _ in 0..count {
            if cursor.remaining() < 24 {
                break;
            }
            
            let block_ssrc = cursor.get_u32();
            let fraction_lost = cursor.get_u8();
            let cumulative_lost = cursor.get_u24();
            let highest_seq = cursor.get_u32();
            let jitter = cursor.get_u32();
            let lsr = cursor.get_u32();
            let dlsr = cursor.get_u32();
            
            report_blocks.push(ReportBlock {
                ssrc: block_ssrc,
                fraction_lost,
                cumulative_lost,
                highest_seq,
                jitter,
                lsr,
                dlsr,
            });
        }
        
        Ok(RtcpPacket::ReceiverReport(ReceiverReport {
            ssrc,
            report_blocks,
        }))
    }
}

// 扩展 Cursor 以支持 get_u24
trait CursorExt {
    fn get_u24(&mut self) -> u32;
}

impl<T: AsRef<[u8]>> CursorExt for std::io::Cursor<T> {
    fn get_u24(&mut self) -> u32 {
        let b1 = self.get_u8() as u32;
        let b2 = self.get_u8() as u32;
        let b3 = self.get_u8() as u32;
        (b1 << 16) | (b2 << 8) | b3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sender_report() {
        // 构造一个简单的 SR 包
        let data = vec![
            0x80, 200, 0x00, 0x06, // V=2, PT=200(SR), length=6
            0x12, 0x34, 0x56, 0x78, // SSRC
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // NTP timestamp
            0x00, 0x00, 0x10, 0x00, // RTP timestamp
            0x00, 0x00, 0x00, 0x64, // Packet count (100)
            0x00, 0x00, 0x27, 0x10, // Octet count (10000)
        ];
        
        let packets = RtcpReceiver::parse_rtcp_packet(&data).unwrap();
        assert_eq!(packets.len(), 1);
        
        match &packets[0] {
            RtcpPacket::SenderReport(sr) => {
                assert_eq!(sr.ssrc, 0x12345678);
                assert_eq!(sr.packet_count, 100);
                assert_eq!(sr.octet_count, 10000);
            }
            _ => panic!("Expected SenderReport"),
        }
    }
}
