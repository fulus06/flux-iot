// GB28181 SIP 服务器
// 处理设备注册、心跳、目录查询、实时点播等

use super::message::{SipMessage, SipMethod, SipRequest, SipResponse};
use super::device::{Device, DeviceManager};
use super::session::{SessionManager, SessionState};
use crate::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

/// SIP 服务器配置
#[derive(Debug, Clone)]
pub struct SipServerConfig {
    /// 监听地址
    pub bind_addr: String,
    
    /// SIP 域
    pub sip_domain: String,
    
    /// SIP ID（平台 ID）
    pub sip_id: String,
    
    /// 设备过期时间（秒）
    pub device_expires: u32,
    
    /// 会话超时时间（秒）
    pub session_timeout: i64,
}

impl Default for SipServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:5060".to_string(),
            sip_domain: "3402000000".to_string(),
            sip_id: "34020000002000000001".to_string(),
            device_expires: 3600,
            session_timeout: 300,
        }
    }
}

/// GB28181 SIP 服务器
pub struct SipServer {
    config: SipServerConfig,
    device_manager: Arc<DeviceManager>,
    session_manager: Arc<SessionManager>,
    socket: Arc<UdpSocket>,
}

impl SipServer {
    /// 创建 SIP 服务器
    pub async fn new(config: SipServerConfig) -> Result<Self> {
        let socket = UdpSocket::bind(&config.bind_addr).await
            .map_err(|e| crate::VideoError::Other(format!("Failed to bind UDP socket: {}", e)))?;
        
        tracing::info!("GB28181 SIP server listening on {}", config.bind_addr);
        
        Ok(Self {
            config,
            device_manager: Arc::new(DeviceManager::new()),
            session_manager: Arc::new(SessionManager::new()),
            socket: Arc::new(socket),
        })
    }
    
    /// 启动服务器
    pub async fn start(self: Arc<Self>) -> Result<()> {
        tracing::info!("GB28181 SIP server started");
        
        // 启动清理任务
        let server_clone = self.clone();
        tokio::spawn(async move {
            server_clone.cleanup_task().await;
        });
        
        // 主接收循环
        let mut buf = vec![0u8; 65536];
        
        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = buf[..len].to_vec();
                    let server = self.clone();
                    
                    // 异步处理消息
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_message(data, addr).await {
                            tracing::error!("Failed to handle message from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to receive UDP packet: {}", e);
                }
            }
        }
    }
    
    /// 处理 SIP 消息
    async fn handle_message(&self, data: Vec<u8>, addr: SocketAddr) -> Result<()> {
        let msg_str = String::from_utf8_lossy(&data);
        
        tracing::debug!("Received SIP message from {}: {}", addr, msg_str);
        
        let message = SipMessage::from_string(&msg_str)
            .map_err(|e| crate::VideoError::Other(format!("Failed to parse SIP message: {}", e)))?;
        
        match message {
            SipMessage::Request(req) => {
                self.handle_request(req, addr).await?;
            }
            SipMessage::Response(resp) => {
                self.handle_response(resp, addr).await?;
            }
        }
        
        Ok(())
    }
    
    /// 处理 SIP 请求
    async fn handle_request(&self, req: SipRequest, addr: SocketAddr) -> Result<()> {
        match req.method {
            SipMethod::Register => {
                self.handle_register(req, addr).await?;
            }
            SipMethod::Message => {
                self.handle_message_method(req, addr).await?;
            }
            SipMethod::Invite => {
                self.handle_invite(req, addr).await?;
            }
            SipMethod::Ack => {
                self.handle_ack(req, addr).await?;
            }
            SipMethod::Bye => {
                self.handle_bye(req, addr).await?;
            }
            _ => {
                tracing::warn!("Unsupported SIP method: {}", req.method);
            }
        }
        
        Ok(())
    }
    
    /// 处理 REGISTER 请求（设备注册）
    async fn handle_register(&self, req: SipRequest, addr: SocketAddr) -> Result<()> {
        tracing::info!("Handling REGISTER from {}", addr);
        
        // 提取设备 ID
        let device_id = self.extract_device_id(&req)?;
        
        // 提取过期时间
        let expires = req.headers.get("Expires")
            .and_then(|e| e.parse::<u32>().ok())
            .unwrap_or(self.config.device_expires);
        
        // 注册设备
        let mut device = Device::new(device_id.clone(), addr.ip().to_string(), addr.port());
        device.expires = expires;
        device.update_keepalive();
        
        self.device_manager.register_device(device).await;
        
        // 发送 200 OK 响应
        let mut response = SipResponse::new(200, "OK".to_string());
        
        // 复制必要的头部
        if let Some(via) = req.headers.get("Via") {
            response.add_header("Via".to_string(), via.clone());
        }
        if let Some(from) = req.headers.get("From") {
            response.add_header("From".to_string(), from.clone());
        }
        if let Some(to) = req.headers.get("To") {
            response.add_header("To".to_string(), to.clone());
        }
        if let Some(call_id) = req.headers.get("Call-ID") {
            response.add_header("Call-ID".to_string(), call_id.clone());
        }
        if let Some(cseq) = req.headers.get("CSeq") {
            response.add_header("CSeq".to_string(), cseq.clone());
        }
        
        response.add_header("Expires".to_string(), expires.to_string());
        response.add_header("Date".to_string(), chrono::Utc::now().to_rfc2822());
        
        self.send_response(response, addr).await?;
        
        tracing::info!("Device registered: {} from {}", device_id, addr);
        
        Ok(())
    }
    
    /// 处理 MESSAGE 请求（心跳、目录查询等）
    async fn handle_message_method(&self, req: SipRequest, addr: SocketAddr) -> Result<()> {
        tracing::debug!("Handling MESSAGE from {}", addr);
        
        let device_id = self.extract_device_id(&req)?;
        
        // 解析 XML 消息体
        if let Some(body) = &req.body {
            if body.contains("<CmdType>Keepalive</CmdType>") {
                // 心跳消息
                self.handle_keepalive(&device_id).await?;
            } else if body.contains("<CmdType>Catalog</CmdType>") {
                // 目录查询响应
                self.handle_catalog_response(&device_id, body).await?;
            }
        }
        
        // 发送 200 OK
        let mut response = SipResponse::new(200, "OK".to_string());
        self.copy_headers(&req, &mut response);
        self.send_response(response, addr).await?;
        
        Ok(())
    }
    
    /// 处理 INVITE 请求（实时点播）
    async fn handle_invite(&self, req: SipRequest, addr: SocketAddr) -> Result<()> {
        tracing::info!("Handling INVITE from {}", addr);
        
        let device_id = self.extract_device_id(&req)?;
        
        // 提取 Call-ID
        let call_id = req.headers.get("Call-ID")
            .ok_or_else(|| crate::VideoError::Other("Missing Call-ID".to_string()))?
            .clone();
        
        // 创建会话
        let session = self.session_manager.create_session(call_id.clone(), device_id.clone()).await;
        
        // 解析 SDP
        if let Some(sdp) = &req.body {
            self.session_manager.set_session_sdp(&call_id, None, Some(sdp.clone())).await;
        }
        
        // 发送 200 OK with SDP
        let mut response = SipResponse::new(200, "OK".to_string());
        self.copy_headers(&req, &mut response);
        
        // 生成本地 SDP
        let local_sdp = self.generate_sdp(&session).await;
        response.set_body(local_sdp);
        response.add_header("Content-Type".to_string(), "application/sdp".to_string());
        
        self.send_response(response, addr).await?;
        
        // 更新会话状态
        self.session_manager.update_session_state(&call_id, SessionState::Established).await;
        
        tracing::info!("INVITE accepted for device {}", device_id);
        
        Ok(())
    }
    
    /// 处理 ACK 请求
    async fn handle_ack(&self, req: SipRequest, _addr: SocketAddr) -> Result<()> {
        tracing::debug!("Handling ACK");
        
        if let Some(call_id) = req.headers.get("Call-ID") {
            self.session_manager.update_session_state(call_id, SessionState::Established).await;
        }
        
        Ok(())
    }
    
    /// 处理 BYE 请求（结束会话）
    async fn handle_bye(&self, req: SipRequest, addr: SocketAddr) -> Result<()> {
        tracing::info!("Handling BYE from {}", addr);
        
        if let Some(call_id) = req.headers.get("Call-ID") {
            self.session_manager.terminate_session(call_id).await;
        }
        
        // 发送 200 OK
        let mut response = SipResponse::new(200, "OK".to_string());
        self.copy_headers(&req, &mut response);
        self.send_response(response, addr).await?;
        
        Ok(())
    }
    
    /// 处理 SIP 响应
    async fn handle_response(&self, _resp: SipResponse, _addr: SocketAddr) -> Result<()> {
        // 处理响应（如果需要）
        Ok(())
    }
    
    /// 处理心跳
    async fn handle_keepalive(&self, device_id: &str) -> Result<()> {
        self.device_manager.update_keepalive(device_id).await;
        tracing::debug!("Keepalive received from device: {}", device_id);
        Ok(())
    }
    
    /// 处理目录查询响应
    async fn handle_catalog_response(&self, device_id: &str, body: &str) -> Result<()> {
        tracing::debug!("Catalog response received from device: {}", device_id);
        
        // 解析目录 XML
        let catalog = super::catalog::parse_gb28181_xml(body)?;
        
        if let Some(device_list) = catalog.device_list {
            tracing::info!(
                "Received catalog from device {}: {} channels",
                device_id,
                device_list.items.len()
            );
            
            // 获取设备并更新通道列表
            if let Some(mut device) = self.device_manager.get_device(device_id).await {
                // 清空现有通道
                device.channels.clear();
                
                // 添加新通道
                for item in device_list.items {
                    let channel = super::device::Channel {
                        channel_id: item.device_id.clone(),
                        name: item.name.clone(),
                        manufacturer: item.manufacturer.clone(),
                        model: item.model.clone(),
                        status: item.status.clone(),
                        parent_id: item.parent_id.clone(),
                        longitude: item.longitude,
                        latitude: item.latitude,
                    };
                    
                    device.add_channel(channel);
                    
                    tracing::debug!(
                        "Added channel: {} - {}",
                        item.device_id,
                        item.name
                    );
                }
                
                // 更新设备
                self.device_manager.register_device(device).await;
            }
        }
        
        Ok(())
    }
    
    /// 生成 SDP
    async fn generate_sdp(&self, session: &super::session::SipSession) -> String {
        let ip = self.config.bind_addr.split(':').next().unwrap_or("0.0.0.0");
        let rtp_port = session.rtp_port.unwrap_or(9000);
        
        let mut sdp_session = super::invite::SdpSession::new(
            self.config.sip_id.clone(),
            ip.to_string(),
        );
        
        sdp_session.add_video(rtp_port);
        
        sdp_session.to_string()
    }
    
    /// 提取设备 ID
    fn extract_device_id(&self, req: &SipRequest) -> Result<String> {
        // 从 From 头部提取设备 ID
        if let Some(from) = req.headers.get("From") {
            if let Some(start) = from.find("sip:") {
                if let Some(end) = from[start + 4..].find('@') {
                    let device_id = &from[start + 4..start + 4 + end];
                    return Ok(device_id.to_string());
                }
            }
        }
        
        Err(crate::VideoError::Other("Failed to extract device ID".to_string()))
    }
    
    /// 复制请求头部到响应
    fn copy_headers(&self, req: &SipRequest, resp: &mut SipResponse) {
        for key in &["Via", "From", "To", "Call-ID", "CSeq"] {
            if let Some(value) = req.headers.get(*key) {
                resp.add_header(key.to_string(), value.clone());
            }
        }
    }
    
    /// 发送响应
    async fn send_response(&self, response: SipResponse, addr: SocketAddr) -> Result<()> {
        let data = response.to_string();
        
        self.socket.send_to(data.as_bytes(), addr).await
            .map_err(|e| crate::VideoError::Other(format!("Failed to send response: {}", e)))?;
        
        tracing::debug!("Sent SIP response to {}: {} {}", addr, response.status_code, response.reason_phrase);
        
        Ok(())
    }
    
    /// 清理任务（定期清理过期设备和会话）
    async fn cleanup_task(&self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // 清理过期设备
            let expired_devices = self.device_manager.cleanup_expired().await;
            if expired_devices > 0 {
                tracing::info!("Cleaned up {} expired devices", expired_devices);
            }
            
            // 清理超时会话
            let timeout_sessions = self.session_manager.cleanup_timeout(self.config.session_timeout).await;
            if timeout_sessions > 0 {
                tracing::info!("Cleaned up {} timeout sessions", timeout_sessions);
            }
        }
    }
    
    /// 获取设备管理器
    pub fn device_manager(&self) -> &Arc<DeviceManager> {
        &self.device_manager
    }
    
    /// 获取会话管理器
    pub fn session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }
    
    /// 发起实时点播（INVITE）
    pub async fn start_realtime_play(
        &self,
        device_id: &str,
        channel_id: &str,
        rtp_port: u16,
    ) -> Result<String> {
        // 获取设备信息
        let device = self.device_manager.get_device(device_id).await
            .ok_or_else(|| crate::VideoError::Other(format!("Device not found: {}", device_id)))?;
        
        // 生成 Call-ID
        let call_id = format!("{}@{}", chrono::Utc::now().timestamp(), self.config.sip_domain);
        
        // 创建会话
        let mut session = self.session_manager.create_session(call_id.clone(), device_id.to_string()).await;
        session.channel_id = Some(channel_id.to_string());
        session.rtp_port = Some(rtp_port);
        session.rtcp_port = Some(rtp_port + 1);
        
        // 生成本地 SDP
        let local_sdp = self.generate_sdp(&session).await;
        self.session_manager.set_session_sdp(&call_id, Some(local_sdp.clone()), None).await;
        
        // 构建 INVITE 请求
        let mut request = SipRequest::new(
            SipMethod::Invite,
            format!("sip:{}@{}:{}", channel_id, device.ip, device.port),
        );
        
        let ip = self.config.bind_addr.split(':').next().unwrap_or("0.0.0.0");
        
        // 添加必要的头部
        request.add_header("Via".to_string(), format!("SIP/2.0/UDP {}:5060;branch=z9hG4bK{}", ip, chrono::Utc::now().timestamp()));
        request.add_header("From".to_string(), format!("<sip:{}@{}>;tag={}", self.config.sip_id, self.config.sip_domain, chrono::Utc::now().timestamp()));
        request.add_header("To".to_string(), format!("<sip:{}@{}>", channel_id, self.config.sip_domain));
        request.add_header("Call-ID".to_string(), call_id.clone());
        request.add_header("CSeq".to_string(), "1 INVITE".to_string());
        request.add_header("Contact".to_string(), format!("<sip:{}@{}:5060>", self.config.sip_id, ip));
        request.add_header("Max-Forwards".to_string(), "70".to_string());
        request.add_header("Subject".to_string(), format!("{}:0,{}:0", channel_id, self.config.sip_id));
        request.add_header("Content-Type".to_string(), "application/sdp".to_string());
        
        // 设置 SDP 消息体
        request.set_body(local_sdp);
        
        // 发送请求
        let addr: SocketAddr = format!("{}:{}", device.ip, device.port).parse()
            .map_err(|e| crate::VideoError::Other(format!("Invalid address: {}", e)))?;
        
        let data = request.to_string();
        self.socket.send_to(data.as_bytes(), addr).await
            .map_err(|e| crate::VideoError::Other(format!("Failed to send INVITE: {}", e)))?;
        
        // 更新会话状态
        self.session_manager.update_session_state(&call_id, SessionState::Calling).await;
        
        tracing::info!("Sent INVITE to device {} channel {}", device_id, channel_id);
        
        Ok(call_id)
    }
    
    /// 停止实时点播（BYE）
    pub async fn stop_realtime_play(&self, call_id: &str) -> Result<()> {
        // 获取会话
        let session = self.session_manager.get_session(call_id).await
            .ok_or_else(|| crate::VideoError::Other(format!("Session not found: {}", call_id)))?;
        
        // 获取设备信息
        let device = self.device_manager.get_device(&session.device_id).await
            .ok_or_else(|| crate::VideoError::Other(format!("Device not found: {}", session.device_id)))?;
        
        // 构建 BYE 请求
        let mut request = SipRequest::new(
            SipMethod::Bye,
            format!("sip:{}@{}:{}", session.channel_id.as_ref().unwrap_or(&session.device_id), device.ip, device.port),
        );
        
        let ip = self.config.bind_addr.split(':').next().unwrap_or("0.0.0.0");
        
        // 添加必要的头部
        request.add_header("Via".to_string(), format!("SIP/2.0/UDP {}:5060", ip));
        request.add_header("From".to_string(), format!("<sip:{}@{}>", self.config.sip_id, self.config.sip_domain));
        request.add_header("To".to_string(), format!("<sip:{}@{}>", session.device_id, self.config.sip_domain));
        request.add_header("Call-ID".to_string(), call_id.to_string());
        request.add_header("CSeq".to_string(), format!("{} BYE", session.cseq + 1));
        request.add_header("Max-Forwards".to_string(), "70".to_string());
        
        // 发送请求
        let addr: SocketAddr = format!("{}:{}", device.ip, device.port).parse()
            .map_err(|e| crate::VideoError::Other(format!("Invalid address: {}", e)))?;
        
        let data = request.to_string();
        self.socket.send_to(data.as_bytes(), addr).await
            .map_err(|e| crate::VideoError::Other(format!("Failed to send BYE: {}", e)))?;
        
        // 终止会话
        self.session_manager.terminate_session(call_id).await;
        
        tracing::info!("Sent BYE for session {}", call_id);
        
        Ok(())
    }
    
    /// 发送目录查询请求
    pub async fn query_catalog(&self, device_id: &str) -> Result<()> {
        // 获取设备信息
        let device = self.device_manager.get_device(device_id).await
            .ok_or_else(|| crate::VideoError::Other(format!("Device not found: {}", device_id)))?;
        
        // 生成序列号
        let sn = chrono::Utc::now().timestamp() as u32;
        
        // 创建目录查询
        let query = super::catalog::CatalogQuery::new(sn, device_id.to_string());
        let xml_body = query.to_xml();
        
        // 构建 SIP MESSAGE 请求
        let mut request = SipRequest::new(
            SipMethod::Message,
            format!("sip:{}@{}:{}", device_id, device.ip, device.port),
        );
        
        // 添加必要的头部
        request.add_header("Via".to_string(), format!("SIP/2.0/UDP {}:5060", self.config.bind_addr.split(':').next().unwrap_or("0.0.0.0")));
        request.add_header("From".to_string(), format!("<sip:{}@{}>", self.config.sip_id, self.config.sip_domain));
        request.add_header("To".to_string(), format!("<sip:{}@{}>", device_id, self.config.sip_domain));
        request.add_header("Call-ID".to_string(), format!("{}@{}", sn, self.config.sip_domain));
        request.add_header("CSeq".to_string(), format!("{} MESSAGE", sn));
        request.add_header("Content-Type".to_string(), "Application/MANSCDP+xml".to_string());
        request.add_header("Max-Forwards".to_string(), "70".to_string());
        
        // 设置消息体
        request.set_body(xml_body);
        
        // 发送请求
        let addr: SocketAddr = format!("{}:{}", device.ip, device.port).parse()
            .map_err(|e| crate::VideoError::Other(format!("Invalid address: {}", e)))?;
        
        let data = request.to_string();
        self.socket.send_to(data.as_bytes(), addr).await
            .map_err(|e| crate::VideoError::Other(format!("Failed to send catalog query: {}", e)))?;
        
        tracing::info!("Sent catalog query to device: {}", device_id);
        
        Ok(())
    }
}
