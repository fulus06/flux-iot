// SIP 会话管理
// 管理 SIP 对话和事务

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// SIP 会话状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Initial,
    Calling,
    Ringing,
    Established,
    Terminated,
}

/// SIP 会话
#[derive(Debug, Clone)]
pub struct SipSession {
    /// 会话 ID（Call-ID）
    pub session_id: String,
    
    /// 设备 ID
    pub device_id: String,
    
    /// 通道 ID
    pub channel_id: Option<String>,
    
    /// 会话状态
    pub state: SessionState,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
    
    /// 本地 SDP
    pub local_sdp: Option<String>,
    
    /// 远端 SDP
    pub remote_sdp: Option<String>,
    
    /// RTP 端口
    pub rtp_port: Option<u16>,
    
    /// RTCP 端口
    pub rtcp_port: Option<u16>,
    
    /// CSeq 序号
    pub cseq: u32,
}

impl SipSession {
    pub fn new(session_id: String, device_id: String) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            device_id,
            channel_id: None,
            state: SessionState::Initial,
            created_at: now,
            updated_at: now,
            local_sdp: None,
            remote_sdp: None,
            rtp_port: None,
            rtcp_port: None,
            cseq: 1,
        }
    }
    
    /// 更新状态
    pub fn update_state(&mut self, state: SessionState) {
        self.state = state;
        self.updated_at = Utc::now();
    }
    
    /// 设置 SDP
    pub fn set_sdp(&mut self, local: Option<String>, remote: Option<String>) {
        self.local_sdp = local;
        self.remote_sdp = remote;
        self.updated_at = Utc::now();
    }
    
    /// 设置 RTP 端口
    pub fn set_rtp_ports(&mut self, rtp: u16, rtcp: u16) {
        self.rtp_port = Some(rtp);
        self.rtcp_port = Some(rtcp);
        self.updated_at = Utc::now();
    }
    
    /// 增加 CSeq
    pub fn next_cseq(&mut self) -> u32 {
        self.cseq += 1;
        self.cseq
    }
    
    /// 检查是否超时
    pub fn is_timeout(&self, timeout_secs: i64) -> bool {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.updated_at);
        elapsed.num_seconds() > timeout_secs
    }
}

/// SIP 会话管理器
pub struct SessionManager {
    /// 会话列表（session_id -> SipSession）
    sessions: Arc<RwLock<HashMap<String, SipSession>>>,
    
    /// 设备会话映射（device_id -> session_id）
    device_sessions: Arc<RwLock<HashMap<String, String>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            device_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 创建会话
    pub async fn create_session(&self, session_id: String, device_id: String) -> SipSession {
        let session = SipSession::new(session_id.clone(), device_id.clone());
        
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session.clone());
        
        let mut device_sessions = self.device_sessions.write().await;
        device_sessions.insert(device_id.clone(), session_id.clone());
        
        tracing::info!("SIP session created: {} for device {}", session_id, device_id);
        
        session
    }
    
    /// 获取会话
    pub async fn get_session(&self, session_id: &str) -> Option<SipSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }
    
    /// 通过设备 ID 获取会话
    pub async fn get_session_by_device(&self, device_id: &str) -> Option<SipSession> {
        let device_sessions = self.device_sessions.read().await;
        
        if let Some(session_id) = device_sessions.get(device_id) {
            let sessions = self.sessions.read().await;
            sessions.get(session_id).cloned()
        } else {
            None
        }
    }
    
    /// 更新会话状态
    pub async fn update_session_state(&self, session_id: &str, state: SessionState) -> bool {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.update_state(state);
            tracing::debug!("Session {} state updated to {:?}", session_id, session.state);
            true
        } else {
            false
        }
    }
    
    /// 设置会话 SDP
    pub async fn set_session_sdp(
        &self,
        session_id: &str,
        local: Option<String>,
        remote: Option<String>,
    ) -> bool {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.set_sdp(local, remote);
            true
        } else {
            false
        }
    }
    
    /// 设置 RTP 端口
    pub async fn set_rtp_ports(&self, session_id: &str, rtp: u16, rtcp: u16) -> bool {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.set_rtp_ports(rtp, rtcp);
            true
        } else {
            false
        }
    }
    
    /// 终止会话
    pub async fn terminate_session(&self, session_id: &str) -> Option<SipSession> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.remove(session_id);
        
        if let Some(ref s) = session {
            let mut device_sessions = self.device_sessions.write().await;
            device_sessions.remove(&s.device_id);
            
            tracing::info!("SIP session terminated: {}", session_id);
        }
        
        session
    }
    
    /// 列出所有会话
    pub async fn list_sessions(&self) -> Vec<SipSession> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }
    
    /// 列出活跃会话
    pub async fn list_active_sessions(&self) -> Vec<SipSession> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| s.state == SessionState::Established)
            .cloned()
            .collect()
    }
    
    /// 清理超时会话
    pub async fn cleanup_timeout(&self, timeout_secs: i64) -> usize {
        let mut sessions = self.sessions.write().await;
        let timeout_ids: Vec<String> = sessions
            .iter()
            .filter(|(_, s)| s.is_timeout(timeout_secs))
            .map(|(id, _)| id.clone())
            .collect();
        
        let count = timeout_ids.len();
        for session_id in timeout_ids {
            if let Some(session) = sessions.remove(&session_id) {
                let mut device_sessions = self.device_sessions.write().await;
                device_sessions.remove(&session.device_id);
                
                tracing::info!("Session timeout and removed: {}", session_id);
            }
        }
        
        count
    }
    
    /// 获取会话数量
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_session_creation() {
        let manager = SessionManager::new();
        
        let session = manager.create_session(
            "test-call-id-123".to_string(),
            "34020000001320000001".to_string(),
        ).await;
        
        assert_eq!(session.session_id, "test-call-id-123");
        assert_eq!(session.device_id, "34020000001320000001");
        assert_eq!(session.state, SessionState::Initial);
    }
    
    #[tokio::test]
    async fn test_session_state_update() {
        let manager = SessionManager::new();
        
        manager.create_session(
            "test-call-id-123".to_string(),
            "34020000001320000001".to_string(),
        ).await;
        
        let updated = manager.update_session_state(
            "test-call-id-123",
            SessionState::Established,
        ).await;
        
        assert!(updated);
        
        let session = manager.get_session("test-call-id-123").await.unwrap();
        assert_eq!(session.state, SessionState::Established);
    }
    
    #[tokio::test]
    async fn test_session_by_device() {
        let manager = SessionManager::new();
        
        manager.create_session(
            "test-call-id-123".to_string(),
            "34020000001320000001".to_string(),
        ).await;
        
        let session = manager.get_session_by_device("34020000001320000001").await;
        assert!(session.is_some());
        assert_eq!(session.unwrap().session_id, "test-call-id-123");
    }
    
    #[tokio::test]
    async fn test_session_termination() {
        let manager = SessionManager::new();
        
        manager.create_session(
            "test-call-id-123".to_string(),
            "34020000001320000001".to_string(),
        ).await;
        
        let terminated = manager.terminate_session("test-call-id-123").await;
        assert!(terminated.is_some());
        
        let session = manager.get_session("test-call-id-123").await;
        assert!(session.is_none());
    }
    
    #[tokio::test]
    async fn test_session_timeout() {
        let manager = SessionManager::new();
        
        let mut session = manager.create_session(
            "test-call-id-123".to_string(),
            "34020000001320000001".to_string(),
        ).await;
        
        // 模拟超时
        session.updated_at = Utc::now() - chrono::Duration::seconds(61);
        
        let mut sessions = manager.sessions.write().await;
        sessions.insert("test-call-id-123".to_string(), session);
        drop(sessions);
        
        let count = manager.cleanup_timeout(60).await;
        assert_eq!(count, 1);
    }
}
