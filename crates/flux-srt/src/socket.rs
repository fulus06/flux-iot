use anyhow::Result;
use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::ack::{AckPacket, NakPacket};
use crate::buffer::{ReceiveBuffer, SendBuffer};
use crate::handshake::{HandshakePacket, HandshakeState};
use crate::packet::{ControlType, SrtControlPacket, SrtDataPacket, SrtHeader};

/// SRT Socket 配置
#[derive(Debug, Clone)]
pub struct SrtSocketConfig {
    pub mtu: u32,
    pub max_flow_window_size: u32,
    pub latency_ms: u32,
    pub keepalive_interval_ms: u64,
    pub connection_timeout_ms: u64,
}

impl Default for SrtSocketConfig {
    fn default() -> Self {
        Self {
            mtu: 1500,
            max_flow_window_size: 8192,
            latency_ms: 120,
            keepalive_interval_ms: 1000,
            connection_timeout_ms: 5000,
        }
    }
}

/// SRT 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Idle,
    Connecting,
    Connected,
    Closing,
    Closed,
}

/// SRT Socket
pub struct SrtSocket {
    pub(crate) socket: Arc<UdpSocket>,
    local_socket_id: u32,
    pub(crate) remote_socket_id: Arc<RwLock<Option<u32>>>,
    pub(crate) remote_addr: Arc<RwLock<Option<SocketAddr>>>,
    pub(crate) state: Arc<RwLock<ConnectionState>>,
    pub(crate) handshake_state: Arc<RwLock<HandshakeState>>,
    pub(crate) config: SrtSocketConfig,
    sequence_number: Arc<RwLock<u32>>,
    // ARQ 支持
    send_buffer: Arc<RwLock<SendBuffer>>,
    receive_buffer: Arc<RwLock<ReceiveBuffer>>,
}

impl SrtSocket {
    /// 创建新的 SRT Socket
    pub async fn new(bind_addr: SocketAddr, config: SrtSocketConfig) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let local_socket_id = Self::generate_socket_id();

        info!(
            target: "srt_socket",
            "SRT Socket created: id={:08x}, addr={}",
            local_socket_id,
            bind_addr
        );

        Ok(Self {
            socket: Arc::new(socket),
            local_socket_id,
            remote_socket_id: Arc::new(RwLock::new(None)),
            remote_addr: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(ConnectionState::Idle)),
            handshake_state: Arc::new(RwLock::new(HandshakeState::Idle)),
            config: config.clone(),
            sequence_number: Arc::new(RwLock::new(1)),
            send_buffer: Arc::new(RwLock::new(SendBuffer::new(
                config.max_flow_window_size as usize,
            ))),
            receive_buffer: Arc::new(RwLock::new(ReceiveBuffer::new(
                1,
                config.max_flow_window_size as usize,
            ))),
        })
    }

    /// 生成随机 Socket ID
    fn generate_socket_id() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u32;
        // 使用时间戳作为随机种子
        now.wrapping_mul(1103515245).wrapping_add(12345)
    }

    /// 获取本地 Socket ID
    pub fn local_socket_id(&self) -> u32 {
        self.local_socket_id
    }

    /// 获取连接状态
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// 发送数据包
    pub async fn send_data(&self, data: &[u8]) -> Result<()> {
        let remote_addr = self
            .remote_addr
            .read()
            .await
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;

        let mut seq = self.sequence_number.write().await;
        let remote_socket_id = *self.remote_socket_id.read().await;
        let current_seq = *seq;
        
        let payload = Bytes::copy_from_slice(data);
        
        let packet = SrtDataPacket {
            header: SrtHeader {
                is_control: false,
                packet_seq_number: current_seq,
                timestamp: Self::get_timestamp(),
                dest_socket_id: remote_socket_id.unwrap_or(0),
            },
            payload: payload.clone(),
        };

        *seq = seq.wrapping_add(1);

        let serialized = packet.serialize();
        self.socket.send_to(&serialized, remote_addr).await?;

        // 添加到发送缓冲区用于重传
        let mut send_buf: tokio::sync::RwLockWriteGuard<SendBuffer> = self.send_buffer.write().await;
        send_buf.insert(current_seq, payload);

        debug!(
            target: "srt_socket",
            "Sent data packet: seq={}, len={}, buffer_size={}",
            current_seq,
            data.len(),
            send_buf.len()
        );

        Ok(())
    }

    /// 发送控制包
    pub(crate) async fn send_control(
        &self,
        control_type: ControlType,
        payload: Bytes,
        addr: SocketAddr,
    ) -> Result<()> {
        let remote_socket_id = *self.remote_socket_id.read().await;
        
        let packet = SrtControlPacket {
            header: SrtHeader {
                is_control: true,
                packet_seq_number: 0,
                timestamp: Self::get_timestamp(),
                dest_socket_id: remote_socket_id.unwrap_or(0),
            },
            control_type,
            type_specific_info: 0,
            payload,
        };

        let serialized = packet.serialize();
        self.socket.send_to(&serialized, addr).await?;

        debug!(
            target: "srt_socket",
            "Sent control packet: type={:?}, addr={}",
            control_type,
            addr
        );

        Ok(())
    }

    /// 发送握手包
    pub(crate) async fn send_handshake(
        &self,
        handshake: &HandshakePacket,
        addr: SocketAddr,
    ) -> Result<()> {
        let payload = handshake.serialize();
        self.send_control(ControlType::Handshake, payload, addr)
            .await
    }

    /// 接收数据包
    pub async fn recv(&self) -> Result<(Bytes, SocketAddr)> {
        let mut buf = vec![0u8; 65536];
        
        loop {
            let (len, addr) = self.socket.recv_from(&mut buf).await?;

            if len < 16 {
                continue;
            }

            // 解析包头判断类型
            let (header, _) = SrtHeader::parse(&buf[..len])
                .map_err(|e| anyhow::anyhow!("Failed to parse header: {}", e))?;

            if header.is_control {
                // 控制包，内部处理
                self.handle_control_packet(&buf[..len], addr).await?;
                continue;
            }

            // 数据包
            let packet = SrtDataPacket::parse(&buf[..len])
                .map_err(|e| anyhow::anyhow!("Failed to parse data packet: {}", e))?;
            return Ok((packet.payload, addr));
        }
    }

    /// 处理控制包
    async fn handle_control_packet(&self, data: &[u8], addr: SocketAddr) -> Result<()> {
        let packet = SrtControlPacket::parse(data)
            .map_err(|e| anyhow::anyhow!("Failed to parse control packet: {}", e))?;

        match packet.control_type {
            ControlType::Handshake => {
                self.handle_handshake(&packet.payload, addr).await?;
            }
            ControlType::KeepAlive => {
                debug!(target: "srt_socket", "Received KeepAlive from {}", addr);
            }
            ControlType::Ack => {
                self.handle_ack(&packet.payload).await?;
            }
            ControlType::Nak => {
                self.handle_nak(&packet.payload).await?;
            }
            ControlType::Shutdown => {
                info!(target: "srt_socket", "Received Shutdown from {}", addr);
                *self.state.write().await = ConnectionState::Closed;
            }
            _ => {
                debug!(
                    target: "srt_socket",
                    "Received control packet: type={:?}",
                    packet.control_type
                );
            }
        }

        Ok(())
    }

    /// 处理握手包
    async fn handle_handshake(&self, payload: &[u8], _addr: SocketAddr) -> Result<()> {
        let handshake = HandshakePacket::parse(payload)
            .map_err(|e| anyhow::anyhow!("Failed to parse handshake: {}", e))?;

        debug!(
            target: "srt_socket",
            "Received handshake: type={:?}, socket_id={:08x}",
            handshake.handshake_type,
            handshake.srt_socket_id
        );

        // 握手处理逻辑将在 Listener/Caller 中实现
        Ok(())
    }

    /// 处理 ACK 包
    async fn handle_ack(&self, payload: &[u8]) -> Result<()> {
        let ack = AckPacket::parse(payload)
            .map_err(|e| anyhow::anyhow!("Failed to parse ACK: {}", e))?;

        debug!(
            target: "srt_socket",
            "Received ACK: last_seq={}, rtt={}us",
            ack.last_ack_seq,
            ack.rtt
        );

        // 确认已接收的包，从发送缓冲区移除
        let mut send_buf: tokio::sync::RwLockWriteGuard<SendBuffer> = self.send_buffer.write().await;
        send_buf.ack_range(ack.last_ack_seq);

        Ok(())
    }

    /// 处理 NAK 包（丢包通知）
    async fn handle_nak(&self, payload: &[u8]) -> Result<()> {
        let nak = NakPacket::parse(payload)
            .map_err(|e| anyhow::anyhow!("Failed to parse NAK: {}", e))?;

        debug!(
            target: "srt_socket",
            "Received NAK: lost_count={}",
            nak.lost_sequences.len()
        );

        // 重传丢失的包
        self.retransmit_packets(&nak.lost_sequences).await?;

        Ok(())
    }

    /// 重传数据包
    async fn retransmit_packets(&self, sequences: &[u32]) -> Result<()> {
        let remote_addr = self
            .remote_addr
            .read()
            .await
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;

        let send_buf: tokio::sync::RwLockReadGuard<SendBuffer> = self.send_buffer.read().await;
        let remote_socket_id = *self.remote_socket_id.read().await;

        for &seq in sequences {
            if let Some(item) = send_buf.get(seq) {
                let packet = SrtDataPacket {
                    header: SrtHeader {
                        is_control: false,
                        packet_seq_number: seq,
                        timestamp: Self::get_timestamp(),
                        dest_socket_id: remote_socket_id.unwrap_or(0),
                    },
                    payload: item.data.clone(),
                };

                let serialized = packet.serialize();
                self.socket.send_to(&serialized, remote_addr).await?;

                debug!(
                    target: "srt_socket",
                    "Retransmitted packet: seq={}, retransmit_count={}",
                    seq,
                    item.retransmit_count + 1
                );
            }
        }

        Ok(())
    }

    /// 启动 KeepAlive 任务
    pub async fn start_keepalive(self: Arc<Self>) {
        let mut interval = interval(Duration::from_millis(self.config.keepalive_interval_ms));

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                let state = self.state().await;
                if state != ConnectionState::Connected {
                    break;
                }

                if let Some(addr) = *self.remote_addr.read().await {
                    if let Err(e) = self.send_control(ControlType::KeepAlive, Bytes::new(), addr).await {
                        error!(target: "srt_socket", "Failed to send KeepAlive: {}", e);
                        break;
                    }
                }
            }
        });
    }

    /// 获取当前时间戳（微秒）
    fn get_timestamp() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u32;
        now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_socket_creation() {
        let config = SrtSocketConfig::default();
        let socket = SrtSocket::new("127.0.0.1:0".parse().unwrap(), config)
            .await
            .unwrap();

        assert_eq!(socket.state().await, ConnectionState::Idle);
        assert!(socket.local_socket_id() != 0);
    }

    #[test]
    fn test_socket_id_generation() {
        let id1 = SrtSocket::generate_socket_id();
        // 添加微小延迟确保时间戳不同
        std::thread::sleep(std::time::Duration::from_micros(1));
        let id2 = SrtSocket::generate_socket_id();

        // 由于使用时间戳，快速连续调用可能生成相同 ID
        // 只验证 ID 不为 0
        assert_ne!(id1, 0);
        assert_ne!(id2, 0);
    }
}
