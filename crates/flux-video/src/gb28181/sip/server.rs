// GB28181 SIP 服务器
// 处理设备注册、心跳、目录查询、实时点播等

use super::device::{Device, DeviceManager};
use super::message::{SipMessage, SipMethod, SipRequest, SipResponse};
use super::session::{SessionManager, SessionState};
use crate::Result;
use md5;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAuthMode {
    None,
    Global,
    PerDevice,
    GlobalOrPerDevice,
}

impl Default for RegisterAuthMode {
    fn default() -> Self {
        Self::None
    }
}

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
    
    /// 全局注册密码（Some 时启用 Digest 鉴权）
    pub auth_password: Option<String>,

    /// REGISTER 鉴权模式（None/Global/PerDevice/GlobalOrPerDevice）
    pub auth_mode: RegisterAuthMode,

    /// 每设备独立密码表（key = device_id）
    pub per_device_passwords: HashMap<String, String>,
}

impl Default for SipServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:5060".to_string(),
            sip_domain: "3402000000".to_string(),
            sip_id: "34020000002000000001".to_string(),
            device_expires: 3600,
            session_timeout: 300,
            auth_password: None,
            auth_mode: RegisterAuthMode::None,
            per_device_passwords: HashMap::new(),
        }
    }
}

/// GB28181 SIP 服务器
pub struct SipServer {
    config: SipServerConfig,
    register_auth: Arc<RwLock<RegisterAuthConfig>>,
    device_manager: Arc<DeviceManager>,
    session_manager: Arc<SessionManager>,
    socket: Arc<UdpSocket>,
}

#[derive(Debug, Clone, Default)]
struct RegisterAuthConfig {
    auth_password: Option<String>,
    auth_mode: RegisterAuthMode,
    per_device_passwords: HashMap<String, String>,
}

impl SipServer {
    /// 创建 SIP 服务器
    pub async fn new(config: SipServerConfig) -> Result<Self> {
        let socket = UdpSocket::bind(&config.bind_addr).await
            .map_err(|e| crate::VideoError::Other(format!("Failed to bind UDP socket: {}", e)))?;
        
        tracing::info!("GB28181 SIP server listening on {}", config.bind_addr);
        
        let register_auth = RegisterAuthConfig {
            auth_password: config.auth_password.clone(),
            auth_mode: config.auth_mode,
            per_device_passwords: config.per_device_passwords.clone(),
        };

        Ok(Self {
            config,
            register_auth: Arc::new(RwLock::new(register_auth)),
            device_manager: Arc::new(DeviceManager::new()),
            session_manager: Arc::new(SessionManager::new()),
            socket: Arc::new(socket),
        })
    }

    pub async fn update_register_auth(
        &self,
        mode: RegisterAuthMode,
        global_password: Option<String>,
        per_device_passwords: HashMap<String, String>,
    ) {
        let mut guard = self.register_auth.write().await;
        guard.auth_mode = mode;
        guard.auth_password = global_password;
        guard.per_device_passwords = per_device_passwords;
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
        let span = tracing::info_span!(
            "gb28181.sip.handle_message",
            remote = %addr,
            bytes = data.len()
        );
        let _enter = span.enter();

        let msg_str = String::from_utf8_lossy(&data);
        
        tracing::debug!(target: "gb28181::sip", "Received SIP message: {}", msg_str);
        
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
                tracing::warn!("Unsupported SIP me.awaitthod: {}", req.method);
            }
        }
        
        Ok(())
    }
    
    /// 处理 REGISTER 请求（设备注册）
    async fn handle_register(&self, req: SipRequest, addr: SocketAddr) -> Result<()> {
        // 提取设备 ID
        let device_id = self.extract_device_id(&req)?;
        
        // 提取过期时间
        let expires = req.headers.get("Expires")
            .and_then(|e| e.parse::<u32>().ok())
            .unwrap_or(self.config.device_expires);
        
        let span = tracing::info_span!(
            "gb28181.sip.register",
            %device_id,
            remote = %addr,
            expires = expires
        );
        let _enter = span.enter();
        
        tracing::info!(target: "gb28181::sip", "Handling REGISTER");
        
        // 如果配置了全局密码，则进行 Digest 鉴权
        let auth_mode = self.effective_auth_mode().await;

        if !matches!(auth_mode, RegisterAuthMode::None) {
            let authorized = self
                .verify_register_auth(&req, &device_id, addr, auth_mode)
                .await?;
            if !authorized {
                // 已返回 401 挑战或错误，终止本次处理
                return Ok(());
            }
        }
        
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
        
        tracing::info!(
            target: "gb28181::sip",
            "Device registered",
        );
        
        Ok(())
    }

    async fn effective_auth_mode(&self) -> RegisterAuthMode {
        let guard = self.register_auth.read().await;
        match guard.auth_mode {
            RegisterAuthMode::None => {
                if guard.auth_password.is_some() {
                    RegisterAuthMode::Global
                } else {
                    RegisterAuthMode::None
                }
            }
            other => other,
        }
    }

    /// 校验 REGISTER Digest 认证
    async fn verify_register_auth(
        &self,
        req: &SipRequest,
        device_id: &str,
        addr: SocketAddr,
        mode: RegisterAuthMode,
    ) -> Result<bool> {
        if matches!(mode, RegisterAuthMode::None) {
            return Ok(true);
        }

        let password_opt: Option<String> = {
            let guard = self.register_auth.read().await;
            match mode {
                RegisterAuthMode::None => None,
                RegisterAuthMode::Global => guard.auth_password.clone(),
                RegisterAuthMode::PerDevice => guard
                    .per_device_passwords
                    .get(device_id)
                    .cloned(),
                RegisterAuthMode::GlobalOrPerDevice => {
                    if let Some(p) = guard.per_device_passwords.get(device_id) {
                        Some(p.clone())
                    } else {
                        guard.auth_password.clone()
                    }
                }
            }
        };

        let Some(password) = password_opt.as_deref() else {
            tracing::warn!(
                target: "gb28181::sip",
                %device_id,
                mode = ?mode,
                "REGISTER auth mode configured but no password found",
            );
            self.send_unauthorized(req, addr, device_id).await?;
            return Ok(false);
        };

        let Some(auth_header) = req.headers.get("Authorization") else {
            self.send_unauthorized(req, addr, device_id).await?;
            return Ok(false);
        };

        let Some(params) = parse_digest_auth_header(auth_header) else {
            tracing::warn!(
                target: "gb28181::sip",
                %device_id,
                "REGISTER Authorization header parse failed",
            );
            self.send_unauthorized(req, addr, device_id).await?;
            return Ok(false);
        };

        let username = params
            .get("username")
            .map(String::as_str)
            .unwrap_or(device_id);
        let realm = params
            .get("realm")
            .map(String::as_str)
            .unwrap_or(self.config.sip_domain.as_str());
        let nonce = match params.get("nonce") {
            Some(v) => v.as_str(),
            None => {
                self.send_unauthorized(req, addr, device_id).await?;
                return Ok(false);
            }
        };
        let uri = params
            .get("uri")
            .map(String::as_str)
            .unwrap_or(req.uri.as_str());
        let response = match params.get("response") {
            Some(v) => v.as_str(),
            None => {
                self.send_unauthorized(req, addr, device_id).await?;
                return Ok(false);
            }
        };

        if username != device_id {
            tracing::warn!(
                target: "gb28181::sip",
                %device_id,
                auth_username = %username,
                "REGISTER username mismatch",
            );
            self.send_unauthorized(req, addr, device_id).await?;
            return Ok(false);
        }

        let method = req.method.to_string();
        let expected = compute_digest_response(
            username,
            realm,
            password,
            &method,
            uri,
            nonce,
        );

        if expected != response {
            tracing::warn!(
                target: "gb28181::sip",
                %device_id,
                "REGISTER digest auth failed",
            );
            self.send_unauthorized(req, addr, device_id).await?;
            return Ok(false);
        }

        tracing::info!(
            target: "gb28181::sip",
            %device_id,
            "REGISTER digest auth success",
        );
        Ok(true)
    }

    /// 发送 401 Unauthorized 挑战
    async fn send_unauthorized(
        &self,
        req: &SipRequest,
        addr: SocketAddr,
        device_id: &str,
    ) -> Result<()> {
        let mut resp = SipResponse::new(401, "Unauthorized".to_string());
        self.copy_headers(req, &mut resp);

        let nonce_source = format!(
            "{}:{}:{}",
            device_id,
            addr,
            chrono::Utc::now().timestamp_millis()
        );
        let nonce = format!("{:x}", md5::compute(nonce_source));

        let realm = &self.config.sip_domain;
        let www_auth = format!(
            "Digest realm=\"{}\", nonce=\"{}\", algorithm=\"MD5\"",
            realm, nonce
        );
        resp.add_header("WWW-Authenticate".to_string(), www_auth);

        self.send_response(resp, addr).await?;

        tracing::warn!(
            target: "gb28181::sip",
            %device_id,
            "Sent 401 Unauthorized for REGISTER",
        );

        Ok(())
    }
    
    /// 处理 MESSAGE 请求（心跳、目录查询等）
    async fn handle_message_method(&self, req: SipRequest, addr: SocketAddr) -> Result<()> {
        let device_id = self.extract_device_id(&req)?;
        
        let span = tracing::info_span!(
            "gb28181.sip.message",
            %device_id,
            remote = %addr
        );
        let _enter = span.enter();
        
        tracing::debug!(target: "gb28181::sip", "Handling MESSAGE");
        
        // 解析 XML 消息体
        if let Some(body) = &req.body {
            if body.contains("<CmdType>Keepalive</CmdType>") {
                // 心跳消息
                self.handle_keepalive(&device_id).await?;
            } else if body.contains("<CmdType>Catalog</CmdType>") {
                // 目录查询响应
                self.handle_catalog_response(&device_id, body).await?;
            } else if body.contains("<CmdType>DeviceInfo</CmdType>") {
                // 设备信息响应
                self.handle_device_info_response(&device_id, body).await?;
            } else if body.contains("<CmdType>DeviceStatus</CmdType>") {
                // 设备状态响应
                self.handle_device_status_response(&device_id, body).await?;
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
        let device_id = self.extract_device_id(&req)?;
        
        let span = tracing::info_span!(
            "gb28181.sip.invite",
            %device_id,
            remote = %addr
        );
        let _enter = span.enter();
        
        tracing::info!(target: "gb28181::sip", "Handling INVITE");
        
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
        
        tracing::info!(target: "gb28181::sip", "INVITE accepted");
        
        Ok(())
    }
    
    /// 处理 ACK 请求
    async fn handle_ack(&self, req: SipRequest, _addr: SocketAddr) -> Result<()> {
        tracing::debug!(target: "gb28181::sip", "Handling ACK");
        
        if let Some(call_id) = req.headers.get("Call-ID") {
            self.session_manager.update_session_state(call_id, SessionState::Established).await;
        }
        
        Ok(())
    }
    
    /// 处理 BYE 请求（结束会话）
    async fn handle_bye(&self, req: SipRequest, addr: SocketAddr) -> Result<()> {
        tracing::info!(target: "gb28181::sip", remote = %addr, "Handling BYE");
        
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
        tracing::debug!(
            target: "gb28181::sip",
            %device_id,
            "Keepalive received from device",
        );
        Ok(())
    }
    
    /// 处理目录查询响应
    async fn handle_catalog_response(&self, device_id: &str, body: &str) -> Result<()> {
        // 解析目录 XML
        let catalog = super::catalog::parse_gb28181_xml(body)?;
        
        let span = tracing::info_span!(
            "gb28181.sip.catalog_response",
            %device_id,
            cmd_type = %catalog.cmd_type,
            sum_num = catalog.sum_num.unwrap_or(0)
        );
        let _enter = span.enter();
        
        tracing::debug!(target: "gb28181::sip", "Catalog response received");
        
        if let Some(device_list) = catalog.device_list {
            tracing::info!(
                target: "gb28181::sip",
                channels = device_list.items.len(),
                "Received catalog from device",
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
                        target: "gb28181::sip",
                        channel_id = %item.device_id,
                        name = %item.name,
                        "Added channel",
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
    
    /// 处理设备信息响应
    async fn handle_device_info_response(&self, device_id: &str, body: &str) -> Result<()> {
        let msg = super::catalog::parse_gb28181_xml(body)?;
        
        let span = tracing::info_span!(
            "gb28181.sip.device_info_response",
            %device_id,
            device_name = %msg.device_name,
            manufacturer = %msg.manufacturer,
            model = %msg.model,
            firmware = %msg.firmware
        );
        let _enter = span.enter();
        
        tracing::debug!(target: "gb28181::sip", "DeviceInfo response received");
        
        // 更新设备信息
        if let Some(mut device) = self.device_manager.get_device(device_id).await {
            if !msg.device_name.is_empty() {
                device.name = msg.device_name.clone();
            }
            if !msg.manufacturer.is_empty() {
                device.manufacturer = msg.manufacturer.clone();
            }
            if !msg.model.is_empty() {
                device.model = msg.model.clone();
            }
            if !msg.firmware.is_empty() {
                device.firmware = msg.firmware.clone();
            }
            
            self.device_manager.register_device(device).await;
            
            tracing::info!(
                target: "gb28181::sip",
                "Updated device info",
            );
        }
        
        Ok(())
    }
    
    /// 处理设备状态响应
    async fn handle_device_status_response(&self, device_id: &str, body: &str) -> Result<()> {
        let msg = super::catalog::parse_gb28181_xml(body)?;
        
        let span = tracing::info_span!(
            "gb28181.sip.device_status_response",
            %device_id,
            online = %msg.online_status,
            device_status = %msg.device_status,
            result = %msg.result
        );
        let _enter = span.enter();
        
        tracing::debug!(target: "gb28181::sip", "DeviceStatus response received");
        
        // 更新设备状态
        if let Some(mut device) = self.device_manager.get_device(device_id).await {
            // 根据 Online 或 Status 字段判断在线状态
            let is_online = msg.online_status.eq_ignore_ascii_case("ONLINE")
                || msg.device_status.eq_ignore_ascii_case("OK")
                || msg.device_status.eq_ignore_ascii_case("ONLINE");
            
            if is_online {
                device.status = super::device::DeviceStatus::Online;
            } else {
                device.status = super::device::DeviceStatus::Offline;
            }
            
            device.update_keepalive();
            self.device_manager.register_device(device).await;
            
            tracing::info!(
                target: "gb28181::sip",
                "Updated device status",
            );
        }
        
        Ok(())
    }
    
    /// 发送目录查询请求
    pub async fn query_catalog(&self, device_id: &str) -> Result<()> {
        // 获取设备信息
        let device = self.device_manager.get_device(device_id).await
            .ok_or_else(|| crate::VideoError::Other(format!("Device not found: {}", device_id)))?;
        
        // 生成序列号
        let sn = chrono::Utc::now().timestamp() as u32;
        let span = tracing::info_span!(
            "gb28181.sip.query_catalog",
            %device_id,
            sn = sn
        );
        let _enter = span.enter();
        
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
        
        tracing::info!(target: "gb28181::sip", "Sent catalog query to device");
        
        Ok(())
    }
    
    /// 发送设备信息查询请求
    pub async fn query_device_info(&self, device_id: &str) -> Result<()> {
        let device = self.device_manager.get_device(device_id).await
            .ok_or_else(|| crate::VideoError::Other(format!("Device not found: {}", device_id)))?;
        
        let sn = chrono::Utc::now().timestamp() as u32;
        let span = tracing::info_span!(
            "gb28181.sip.query_device_info",
            %device_id,
            sn = sn
        );
        let _enter = span.enter();
        
        let xml_body = format!(
            r#"<?xml version="1.0" encoding="GB2312"?>
<Query>
<CmdType>DeviceInfo</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
            sn, device_id
        );
        
        self.send_manscdp_message(device_id, &device, sn, &xml_body).await?;
        
        tracing::info!(target: "gb28181::sip", "Sent DeviceInfo query to device");
        Ok(())
    }
    
    /// 发送设备状态查询请求
    pub async fn query_device_status(&self, device_id: &str) -> Result<()> {
        let device = self.device_manager.get_device(device_id).await
            .ok_or_else(|| crate::VideoError::Other(format!("Device not found: {}", device_id)))?;
        
        let sn = chrono::Utc::now().timestamp() as u32;
        let span = tracing::info_span!(
            "gb28181.sip.query_device_status",
            %device_id,
            sn = sn
        );
        let _enter = span.enter();
        
        let xml_body = format!(
            r#"<?xml version="1.0" encoding="GB2312"?>
<Query>
<CmdType>DeviceStatus</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
            sn, device_id
        );
        
        self.send_manscdp_message(device_id, &device, sn, &xml_body).await?;
        
        tracing::info!(target: "gb28181::sip", "Sent DeviceStatus query to device");
        Ok(())
    }
    
    /// 发送 MANSCDP XML 消息的通用方法
    async fn send_manscdp_message(
        &self,
        device_id: &str,
        device: &Device,
        sn: u32,
        xml_body: &str,
    ) -> Result<()> {
        let mut request = SipRequest::new(
            SipMethod::Message,
            format!("sip:{}@{}:{}", device_id, device.ip, device.port),
        );
        
        let local_ip = self.config.bind_addr.split(':').next().unwrap_or("0.0.0.0");
        
        request.add_header("Via".to_string(), format!("SIP/2.0/UDP {}:5060", local_ip));
        request.add_header("From".to_string(), format!("<sip:{}@{}>", self.config.sip_id, self.config.sip_domain));
        request.add_header("To".to_string(), format!("<sip:{}@{}>", device_id, self.config.sip_domain));
        request.add_header("Call-ID".to_string(), format!("{}@{}", sn, self.config.sip_domain));
        request.add_header("CSeq".to_string(), format!("{} MESSAGE", sn));
        request.add_header("Content-Type".to_string(), "Application/MANSCDP+xml".to_string());
        request.add_header("Max-Forwards".to_string(), "70".to_string());
        
        request.set_body(xml_body.to_string());
        
        let addr: SocketAddr = format!("{}:{}", device.ip, device.port).parse()
            .map_err(|e| crate::VideoError::Other(format!("Invalid address: {}", e)))?;
        
        let data = request.to_string();
        self.socket.send_to(data.as_bytes(), addr).await
            .map_err(|e| crate::VideoError::Other(format!("Failed to send message: {}", e)))?;
        
        Ok(())
    }
}

/// 解析 Digest Authorization / WWW-Authenticate 头部为键值对
fn parse_digest_auth_header(value: &str) -> Option<HashMap<String, String>> {
    let prefix = "Digest ";
    let rest = if let Some(stripped) = value.strip_prefix(prefix) {
        stripped
    } else {
        value
    };

    let mut map = HashMap::new();

    for part in rest.split(',') {
        let trimmed = part.trim();
        if let Some(eq_idx) = trimmed.find('=') {
            let key = trimmed[..eq_idx].trim().to_string();
            let mut val = trimmed[eq_idx + 1..].trim().to_string();
            if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                val = val[1..val.len() - 1].to_string();
            }
            map.insert(key, val);
        }
    }

    if map.is_empty() { None } else { Some(map) }
}

/// 计算 HTTP Digest 响应（简化版，不使用 qop）
fn compute_digest_response(
    username: &str,
    realm: &str,
    password: &str,
    method: &str,
    uri: &str,
    nonce: &str,
) -> String {
    let ha1_source = format!("{}:{}:{}", username, realm, password);
    let ha1 = format!("{:x}", md5::compute(ha1_source));

    let ha2_source = format!("{}:{}", method, uri);
    let ha2 = format!("{:x}", md5::compute(ha2_source));

    let resp_source = format!("{}:{}:{}", ha1, nonce, ha2);
    format!("{:x}", md5::compute(resp_source))
}
