use anyhow::Result;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::rtp_receiver::RtpPacket;

/// 多播接收器
pub struct MulticastReceiver {
    socket: UdpSocket,
    multicast_addr: Ipv4Addr,
    port: u16,
    tx: mpsc::Sender<RtpPacket>,
}

impl MulticastReceiver {
    /// 创建多播接收器
    /// 
    /// # 参数
    /// - `multicast_addr`: 多播地址（224.0.0.0 - 239.255.255.255）
    /// - `port`: 多播端口
    pub async fn new(
        multicast_addr: Ipv4Addr,
        port: u16,
    ) -> Result<(Self, mpsc::Receiver<RtpPacket>)> {
        // 验证多播地址范围
        if !Self::is_multicast_address(&multicast_addr) {
            return Err(anyhow::anyhow!(
                "Invalid multicast address: {}. Must be in range 224.0.0.0 - 239.255.255.255",
                multicast_addr
            ));
        }
        
        // 绑定到 0.0.0.0:port（接收所有接口的多播数据）
        let bind_addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let socket = UdpSocket::bind(bind_addr).await?;
        
        // 加入多播组（IGMP Join）
        socket.join_multicast_v4(multicast_addr, Ipv4Addr::UNSPECIFIED)?;
        
        info!(
            target: "multicast_receiver",
            "Multicast receiver joined group {}:{} on all interfaces",
            multicast_addr, port
        );
        
        let (tx, rx) = mpsc::channel(100);
        
        Ok((
            Self {
                socket,
                multicast_addr,
                port,
                tx,
            },
            rx,
        ))
    }
    
    /// 创建多播接收器（指定网络接口）
    pub async fn new_with_interface(
        multicast_addr: Ipv4Addr,
        port: u16,
        interface: Ipv4Addr,
    ) -> Result<(Self, mpsc::Receiver<RtpPacket>)> {
        if !Self::is_multicast_address(&multicast_addr) {
            return Err(anyhow::anyhow!(
                "Invalid multicast address: {}",
                multicast_addr
            ));
        }
        
        let bind_addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let socket = UdpSocket::bind(bind_addr).await?;
        
        // 加入多播组（指定接口）
        socket.join_multicast_v4(multicast_addr, interface)?;
        
        info!(
            target: "multicast_receiver",
            "Multicast receiver joined group {}:{} on interface {}",
            multicast_addr, port, interface
        );
        
        let (tx, rx) = mpsc::channel(100);
        
        Ok((
            Self {
                socket,
                multicast_addr,
                port,
                tx,
            },
            rx,
        ))
    }
    
    /// 验证是否为有效的多播地址
    fn is_multicast_address(addr: &Ipv4Addr) -> bool {
        let octets = addr.octets();
        // 多播地址范围：224.0.0.0 - 239.255.255.255
        // 第一个字节：224-239 (0xE0-0xEF)
        octets[0] >= 224 && octets[0] <= 239
    }
    
    /// 开始接收多播数据
    pub async fn start(mut self) {
        let mut buffer = vec![0u8; 2048];
        
        info!(
            target: "multicast_receiver",
            "Starting multicast receiver for {}:{}",
            self.multicast_addr, self.port
        );
        
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, src_addr)) => {
                    if len < 12 {
                        continue; // RTP 头部至少 12 字节
                    }
                    
                    debug!(
                        target: "multicast_receiver",
                        "Received {} bytes from {} on multicast group {}:{}",
                        len, src_addr, self.multicast_addr, self.port
                    );
                    
                    match Self::parse_rtp_packet(&buffer[..len]) {
                        Ok(packet) => {
                            if let Err(e) = self.tx.send(packet).await {
                                error!(
                                    target: "multicast_receiver",
                                    "Failed to send RTP packet: {}", e
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            error!(
                                target: "multicast_receiver",
                                "Failed to parse RTP packet: {}", e
                            );
                        }
                    }
                }
                Err(e) => {
                    error!(
                        target: "multicast_receiver",
                        "Failed to receive UDP packet: {}", e
                    );
                    break;
                }
            }
        }
        
        // 离开多播组
        if let Err(e) = self.socket.leave_multicast_v4(
            self.multicast_addr,
            Ipv4Addr::UNSPECIFIED,
        ) {
            error!(
                target: "multicast_receiver",
                "Failed to leave multicast group: {}", e
            );
        } else {
            info!(
                target: "multicast_receiver",
                "Left multicast group {}:{}",
                self.multicast_addr, self.port
            );
        }
    }
    
    /// 解析 RTP 包（复用 RtpReceiver 的逻辑）
    fn parse_rtp_packet(data: &[u8]) -> Result<RtpPacket> {
        use bytes::Buf;
        
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
        
        let payload = bytes::Bytes::copy_from_slice(&data[payload_start..payload_end]);
        
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
    fn test_is_multicast_address() {
        // 有效的多播地址
        assert!(MulticastReceiver::is_multicast_address(&Ipv4Addr::new(224, 0, 0, 1)));
        assert!(MulticastReceiver::is_multicast_address(&Ipv4Addr::new(239, 255, 255, 255)));
        assert!(MulticastReceiver::is_multicast_address(&Ipv4Addr::new(230, 1, 2, 3)));
        
        // 无效的多播地址
        assert!(!MulticastReceiver::is_multicast_address(&Ipv4Addr::new(192, 168, 1, 1)));
        assert!(!MulticastReceiver::is_multicast_address(&Ipv4Addr::new(10, 0, 0, 1)));
        assert!(!MulticastReceiver::is_multicast_address(&Ipv4Addr::new(223, 255, 255, 255)));
        assert!(!MulticastReceiver::is_multicast_address(&Ipv4Addr::new(240, 0, 0, 1)));
    }
}
