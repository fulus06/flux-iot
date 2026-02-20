use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tracing::{debug, info};

use crate::handshake::{HandshakePacket, HandshakeState, HandshakeType};
use crate::packet::ControlType;
use crate::socket::{ConnectionState, SrtSocket, SrtSocketConfig};

/// SRT Caller（客户端模式）
pub struct SrtCaller {
    socket: Arc<SrtSocket>,
    remote_addr: SocketAddr,
}

impl SrtCaller {
    /// 创建 Caller
    pub async fn new(remote_addr: SocketAddr) -> Result<Self> {
        let config = SrtSocketConfig::default();
        let socket = SrtSocket::new("0.0.0.0:0".parse().unwrap(), config).await?;

        info!(
            target: "srt_caller",
            "SRT Caller created: local_id={:08x}, remote={}",
            socket.local_socket_id(),
            remote_addr
        );

        Ok(Self {
            socket: Arc::new(socket),
            remote_addr,
        })
    }

    /// 连接到服务器
    pub async fn connect(self) -> Result<Arc<SrtSocket>> {
        info!(target: "srt_caller", "Connecting to {}", self.remote_addr);

        *self.socket.state.write().await = ConnectionState::Connecting;
        *self.socket.handshake_state.write().await = HandshakeState::Idle;

        // 第一次握手：发送 Induction 请求
        let induction_req =
            HandshakePacket::create_induction_request(self.socket.local_socket_id());
        self.socket
            .send_handshake(&induction_req, self.remote_addr)
            .await?;

        *self.socket.handshake_state.write().await = HandshakeState::InductionSent;

        debug!(target: "srt_caller", "Sent Induction request");

        // 等待第二次握手：接收 Induction 响应
        let induction_resp = self.wait_for_handshake(HandshakeType::Agreement).await?;

        debug!(
            target: "srt_caller",
            "Received Induction response: cookie={:08x}",
            induction_resp.syn_cookie
        );

        // 第三次握手：发送 Conclusion 请求
        let conclusion_req = HandshakePacket::create_conclusion_request(
            self.socket.local_socket_id(),
            induction_resp.syn_cookie,
            1000, // 初始序列号
        );
        self.socket
            .send_handshake(&conclusion_req, self.remote_addr)
            .await?;

        *self.socket.handshake_state.write().await = HandshakeState::ConclusionSent;

        debug!(target: "srt_caller", "Sent Conclusion request");

        // 等待第四次握手：接收 Conclusion 响应
        let _conclusion_resp = self.wait_for_handshake(HandshakeType::Agreement).await?;

        debug!(target: "srt_caller", "Received Conclusion response");

        // 连接建立
        *self.socket.remote_addr.write().await = Some(self.remote_addr);
        *self.socket.remote_socket_id.write().await = Some(induction_resp.srt_socket_id);
        *self.socket.state.write().await = ConnectionState::Connected;
        *self.socket.handshake_state.write().await = HandshakeState::Connected;

        info!(target: "srt_caller", "Connection established");

        // 启动 KeepAlive
        self.socket.clone().start_keepalive().await;

        Ok(self.socket)
    }

    /// 等待握手响应
    async fn wait_for_handshake(
        &self,
        expected_type: HandshakeType,
    ) -> Result<HandshakePacket> {
        let timeout_duration = Duration::from_millis(self.socket.config.connection_timeout_ms);

        timeout(timeout_duration, async {
            loop {
                let mut buf = vec![0u8; 65536];
                let (len, _addr) = self.socket.socket.recv_from(&mut buf).await?;

                if len < 16 {
                    continue;
                }

                // 解析包头
                let (header, _) = crate::packet::SrtHeader::parse(&buf[..len])
                    .map_err(|e| anyhow::anyhow!("Failed to parse header: {}", e))?;

                if !header.is_control {
                    continue;
                }

                // 解析控制包
                let control_packet = crate::packet::SrtControlPacket::parse(&buf[..len])
                    .map_err(|e| anyhow::anyhow!("Failed to parse control packet: {}", e))?;

                if control_packet.control_type == ControlType::Handshake {
                    let handshake = HandshakePacket::parse(&control_packet.payload)
                        .map_err(|e| anyhow::anyhow!("Failed to parse handshake: {}", e))?;

                    if handshake.handshake_type == expected_type {
                        return Ok(handshake);
                    }
                }
            }
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_caller_creation() {
        let caller = SrtCaller::new("127.0.0.1:9000".parse().unwrap())
            .await
            .unwrap();

        assert!(caller.socket.local_socket_id() != 0);
        assert_eq!(caller.remote_addr.port(), 9000);
    }
}
