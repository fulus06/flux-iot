use crate::types::CoapConfig;
use coap_lite::{CoapRequest, CoapOption, MessageClass, Packet, RequestType as Method};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// 订阅回调类型
type ObserveCallback = Arc<dyn Fn(Vec<u8>) + Send + Sync>;

/// 订阅信息
struct SubscriptionInfo {
    path: String,
    callback: ObserveCallback,
}

/// CoAP 客户端
pub struct CoapClient {
    config: CoapConfig,
    socket: Option<UdpSocket>,
    server_addr: Option<SocketAddr>,
    /// Observe 订阅管理 (token -> subscription info)
    observe_subscriptions: Arc<RwLock<HashMap<Vec<u8>, SubscriptionInfo>>>,
    /// 后台任务取消通知
    observe_cancel_tx: Option<mpsc::Sender<()>>,
}

impl CoapClient {
    /// 创建新的 CoAP 客户端
    pub fn new(config: CoapConfig) -> Self {
        Self {
            config,
            socket: None,
            server_addr: None,
            observe_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            observe_cancel_tx: None,
        }
    }

    /// 连接到 CoAP 服务器
    pub async fn connect(&mut self) -> anyhow::Result<()> {
        let server_addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_millis(self.config.timeout_ms)))?;
        socket.set_write_timeout(Some(Duration::from_millis(self.config.timeout_ms)))?;
        
        self.socket = Some(socket);
        self.server_addr = Some(server_addr);
        
        info!(
            host = %self.config.host,
            port = %self.config.port,
            "Connected to CoAP server"
        );
        
        Ok(())
    }

    /// 断开连接
    pub async fn disconnect(&mut self) -> anyhow::Result<()> {
        // 取消所有 Observe 订阅
        if let Some(tx) = self.observe_cancel_tx.take() {
            let _ = tx.send(()).await;
        }
        
        self.observe_subscriptions.write().await.clear();
        self.socket = None;
        self.server_addr = None;
        debug!("Disconnected from CoAP server");
        Ok(())
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.socket.is_some() && self.server_addr.is_some()
    }

    /// GET 请求
    pub async fn get(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        self.request(Method::Get, path, vec![]).await
    }

    /// PUT 请求
    pub async fn put(&self, path: &str, payload: Vec<u8>) -> anyhow::Result<()> {
        self.request(Method::Put, path, payload).await?;
        Ok(())
    }

    /// POST 请求
    pub async fn post(&self, path: &str, payload: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        self.request(Method::Post, path, payload).await
    }

    /// DELETE 请求
    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        self.request(Method::Delete, path, vec![]).await?;
        Ok(())
    }

    /// 发送 CoAP 请求
    async fn request(&self, method: Method, path: &str, payload: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        let server_addr = self.server_addr
            .ok_or_else(|| anyhow::anyhow!("Server address not set"))?;

        // 创建 CoAP 请求
        let mut request: CoapRequest<SocketAddr> = CoapRequest::new();
        request.set_method(method);
        request.set_path(path);
        
        if !payload.is_empty() {
            request.message.payload = payload;
        }

        let packet = request.message.to_bytes()?;

        // 发送请求
        socket.send_to(&packet, server_addr)?;
        
        debug!(
            method = ?method,
            path = %path,
            "Sent CoAP request"
        );

        // 接收响应
        let mut buf = [0; 1024];
        let (size, _) = socket.recv_from(&mut buf)?;
        
        let response = Packet::from_bytes(&buf[..size])?;
        
        debug!(
            code = %response.header.code,
            "Received CoAP response"
        );
        
        Ok(response.payload)
    }
    
    /// 订阅资源（CoAP Observe）
    pub async fn observe<F>(
        &mut self,
        path: &str,
        callback: F,
    ) -> anyhow::Result<Vec<u8>>
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        let socket = self.socket.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        let server_addr = self.server_addr
            .ok_or_else(|| anyhow::anyhow!("Server address not set"))?;

        // 创建带 Observe 选项的 GET 请求
        let mut request: CoapRequest<SocketAddr> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(path);
        
        // 添加 Observe 选项 (Option 6, value 0 = register)
        request.message.add_option(CoapOption::Observe, vec![0]);
        
        // 生成唯一 token
        let token = Self::generate_token();
        request.message.header.message_id = rand::random();
        request.message.set_token(token.clone());

        let packet = request.message.to_bytes()?;

        // 发送 Observe 请求
        socket.send_to(&packet, server_addr)?;
        
        info!(
            path = %path,
            token = ?token,
            "Sent CoAP Observe request"
        );

        // 接收初始响应
        let mut buf = [0; 1024];
        let (size, _) = socket.recv_from(&mut buf)?;
        let response = Packet::from_bytes(&buf[..size])?;
        
        // 注册回调和路径信息
        let callback_arc = Arc::new(callback);
        let sub_info = SubscriptionInfo {
            path: path.to_string(),
            callback: callback_arc,
        };
        self.observe_subscriptions.write().await.insert(token.clone(), sub_info);
        
        // 启动后台任务接收通知（如果还没启动）
        if self.observe_cancel_tx.is_none() {
            self.start_observe_listener().await?;
        }
        
        Ok(token)
    }
    
    /// 取消订阅
    pub async fn cancel_observe(&mut self, token: &[u8]) -> anyhow::Result<()> {
        // 从订阅列表中移除
        let subscription = self.observe_subscriptions.write().await.remove(token);
        
        if let Some(sub) = subscription {
            let socket = self.socket.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
            let server_addr = self.server_addr
                .ok_or_else(|| anyhow::anyhow!("Server address not set"))?;
            
            // 发送 RST (Reset) 消息取消 Observe
            // 根据 RFC 7641，客户端可以通过发送 RST 消息来取消 Observe 订阅
            let mut message: CoapRequest<SocketAddr> = CoapRequest::new();
            message.set_method(Method::Get);
            message.set_path(&sub.path);
            message.message.header.set_type(coap_lite::MessageType::Reset);
            message.message.set_token(token.to_vec());
            
            // 发送 RST 消息
            let packet = message.message.to_bytes()?;
            socket.send_to(&packet, &server_addr)?;
            
            info!(
                token = ?token,
                path = %sub.path,
                "Sent RST message to cancel CoAP Observe subscription"
            );
        } else {
            warn!(token = ?token, "Observe subscription not found");
        }
        
        Ok(())
    }
    
    /// 启动后台监听任务
    async fn start_observe_listener(&mut self) -> anyhow::Result<()> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        
        // 克隆 socket（需要转换为非阻塞）
        let listen_socket = socket.try_clone()?;
        listen_socket.set_nonblocking(true)?;
        
        let subscriptions = self.observe_subscriptions.clone();
        let (cancel_tx, mut cancel_rx) = mpsc::channel(1);
        self.observe_cancel_tx = Some(cancel_tx);
        
        tokio::spawn(async move {
            let mut buf = [0; 1024];
            
            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        debug!("CoAP Observe listener cancelled");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // 尝试接收通知
                        match listen_socket.recv_from(&mut buf) {
                            Ok((size, _)) => {
                                if let Ok(packet) = Packet::from_bytes(&buf[..size]) {
                                    let token = packet.get_token();
                                    
                                    // 查找对应的回调
                                    let subs = subscriptions.read().await;
                                    if let Some(sub_info) = subs.get(token) {
                                        debug!(
                                            token = ?token,
                                            payload_size = packet.payload.len(),
                                            "Received CoAP Observe notification"
                                        );
                                        (sub_info.callback)(packet.payload.clone());
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // 没有数据，继续等待
                            }
                            Err(e) => {
                                error!(error = %e, "Error receiving CoAP notification");
                            }
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// 生成随机 token
    fn generate_token() -> Vec<u8> {
        let token_value: u32 = rand::random();
        token_value.to_be_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coap_client_creation() {
        let config = CoapConfig::default();
        let client = CoapClient::new(config);
        assert!(!client.is_connected());
    }
}
