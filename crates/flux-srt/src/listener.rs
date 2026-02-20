use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::handshake::{HandshakePacket, HandshakeState, HandshakeType};
use crate::packet::ControlType;
use crate::socket::{ConnectionState, SrtSocket, SrtSocketConfig};

/// SRT Listener（服务器模式）
pub struct SrtListener {
    socket: Arc<SrtSocket>,
    pending_connections: Arc<RwLock<HashMap<SocketAddr, PendingConnection>>>,
    accept_tx: mpsc::Sender<Arc<SrtSocket>>,
    accept_rx: Option<mpsc::Receiver<Arc<SrtSocket>>>,
}

/// 待处理的连接
struct PendingConnection {
    syn_cookie: u32,
    handshake_state: HandshakeState,
}

impl SrtListener {
    /// 创建 Listener
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let config = SrtSocketConfig::default();
        let socket = SrtSocket::new(addr, config).await?;

        let (accept_tx, accept_rx) = mpsc::channel(10);

        info!(target: "srt_listener", "SRT Listener bound to {}", addr);

        Ok(Self {
            socket: Arc::new(socket),
            pending_connections: Arc::new(RwLock::new(HashMap::new())),
            accept_tx,
            accept_rx: Some(accept_rx),
        })
    }

    /// 启动监听
    pub async fn start(mut self) -> Result<mpsc::Receiver<Arc<SrtSocket>>> {
        let accept_rx = self
            .accept_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("Listener already started"))?;

        let socket = self.socket.clone();
        let pending = self.pending_connections.clone();
        let accept_tx = self.accept_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::listen_loop(socket, pending, accept_tx).await {
                warn!(target: "srt_listener", "Listen loop error: {}", e);
            }
        });

        Ok(accept_rx)
    }

    /// 监听循环
    async fn listen_loop(
        socket: Arc<SrtSocket>,
        pending: Arc<RwLock<HashMap<SocketAddr, PendingConnection>>>,
        accept_tx: mpsc::Sender<Arc<SrtSocket>>,
    ) -> Result<()> {
        let mut buf = vec![0u8; 65536];

        loop {
            let (len, addr) = socket.socket.recv_from(&mut buf).await?;

            if len < 16 {
                continue;
            }

            // 解析包头
            let (header, _) = crate::packet::SrtHeader::parse(&buf[..len])
                .map_err(|e| anyhow::anyhow!("Failed to parse header: {}", e))?;

            if !header.is_control {
                // 数据包，忽略（连接建立后才处理）
                continue;
            }

            // 解析控制包
            let control_packet = crate::packet::SrtControlPacket::parse(&buf[..len])
                .map_err(|e| anyhow::anyhow!("Failed to parse control packet: {}", e))?;

            if control_packet.control_type == ControlType::Handshake {
                Self::handle_handshake(
                    &socket,
                    &pending,
                    &accept_tx,
                    &control_packet.payload,
                    addr,
                )
                .await?;
            }
        }
    }

    /// 处理握手包
    async fn handle_handshake(
        socket: &Arc<SrtSocket>,
        pending: &Arc<RwLock<HashMap<SocketAddr, PendingConnection>>>,
        accept_tx: &mpsc::Sender<Arc<SrtSocket>>,
        payload: &[u8],
        addr: SocketAddr,
    ) -> Result<()> {
        let handshake = HandshakePacket::parse(payload)
            .map_err(|e| anyhow::anyhow!("Failed to parse handshake: {}", e))?;

        debug!(
            target: "srt_listener",
            "Handshake from {}: type={:?}, socket_id={:08x}",
            addr,
            handshake.handshake_type,
            handshake.srt_socket_id
        );

        match handshake.handshake_type {
            HandshakeType::Induction => {
                // 第一次握手：生成 SYN Cookie 并响应
                let syn_cookie = Self::generate_syn_cookie();
                let response = HandshakePacket::create_induction_response(
                    &handshake,
                    socket.local_socket_id(),
                    syn_cookie,
                );

                socket.send_handshake(&response, addr).await?;

                // 记录待处理连接
                let mut pending_map = pending.write().await;
                pending_map.insert(
                    addr,
                    PendingConnection {
                        syn_cookie,
                        handshake_state: HandshakeState::InductionReceived,
                    },
                );

                info!(
                    target: "srt_listener",
                    "Sent Induction response to {}, cookie={:08x}",
                    addr,
                    syn_cookie
                );
            }
            HandshakeType::Conclusion => {
                // 第三次握手：验证 SYN Cookie
                let mut pending_map = pending.write().await;

                if let Some(pending_conn) = pending_map.get(&addr) {
                    if pending_conn.syn_cookie == handshake.syn_cookie {
                        // Cookie 验证通过，发送最终响应
                        let response = HandshakePacket::create_conclusion_response(
                            &handshake,
                            socket.local_socket_id(),
                        );

                        socket.send_handshake(&response, addr).await?;

                        // 创建新的连接 Socket
                        let config = SrtSocketConfig::default();
                        let new_socket = SrtSocket::new("0.0.0.0:0".parse().unwrap(), config).await?;
                        *new_socket.remote_addr.write().await = Some(addr);
                        *new_socket.remote_socket_id.write().await = Some(handshake.srt_socket_id);
                        *new_socket.state.write().await = ConnectionState::Connected;

                        let new_socket = Arc::new(new_socket);

                        // 启动 KeepAlive
                        new_socket.clone().start_keepalive().await;

                        // 通知接受新连接
                        if accept_tx.send(new_socket).await.is_err() {
                            warn!(target: "srt_listener", "Failed to accept connection from {}", addr);
                        } else {
                            info!(target: "srt_listener", "Connection established with {}", addr);
                        }

                        // 移除待处理连接
                        pending_map.remove(&addr);
                    } else {
                        warn!(
                            target: "srt_listener",
                            "Invalid SYN cookie from {}: expected={:08x}, got={:08x}",
                            addr,
                            pending_conn.syn_cookie,
                            handshake.syn_cookie
                        );
                    }
                } else {
                    warn!(target: "srt_listener", "No pending connection for {}", addr);
                }
            }
            _ => {
                debug!(
                    target: "srt_listener",
                    "Unexpected handshake type: {:?}",
                    handshake.handshake_type
                );
            }
        }

        Ok(())
    }

    /// 生成 SYN Cookie
    fn generate_syn_cookie() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u32;
        now.wrapping_mul(1664525).wrapping_add(1013904223)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_listener_creation() {
        let listener = SrtListener::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();

        assert!(listener.socket.local_socket_id() != 0);
    }
}
