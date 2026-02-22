use crate::types::CoapConfig;
use coap_lite::{CoapRequest, MessageClass, Packet, RequestType as Method};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;
use tracing::{debug, info};

/// CoAP 客户端
pub struct CoapClient {
    config: CoapConfig,
    socket: Option<UdpSocket>,
    server_addr: Option<SocketAddr>,
}

impl CoapClient {
    /// 创建新的 CoAP 客户端
    pub fn new(config: CoapConfig) -> Self {
        Self {
            config,
            socket: None,
            server_addr: None,
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
