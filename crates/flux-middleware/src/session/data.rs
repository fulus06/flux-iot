use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 会话数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// 会话 ID
    pub session_id: String,
    
    /// 用户 ID
    pub user_id: String,
    
    /// 流 ID（可选）
    pub stream_id: Option<String>,
    
    /// 协议类型
    pub protocol: Option<String>,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 最后活跃时间
    pub last_active: DateTime<Utc>,
    
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl SessionData {
    /// 创建新的会话数据
    pub fn new(user_id: String) -> Self {
        let now = Utc::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            stream_id: None,
            protocol: None,
            created_at: now,
            last_active: now,
            metadata: HashMap::new(),
        }
    }

    /// 更新最后活跃时间
    pub fn touch(&mut self) {
        self.last_active = Utc::now();
    }

    /// 设置流 ID
    pub fn with_stream(mut self, stream_id: String) -> Self {
        self.stream_id = Some(stream_id);
        self
    }

    /// 设置协议
    pub fn with_protocol(mut self, protocol: String) -> Self {
        self.protocol = Some(protocol);
        self
    }

    /// 添加元数据
    pub fn add_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// 检查会话是否过期
    pub fn is_expired(&self, ttl_seconds: i64) -> bool {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.last_active);
        duration.num_seconds() > ttl_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_data_creation() {
        let session = SessionData::new("user123".to_string())
            .with_stream("stream456".to_string())
            .with_protocol("rtmp".to_string())
            .add_metadata("ip".to_string(), "192.168.1.1".to_string());

        assert_eq!(session.user_id, "user123");
        assert_eq!(session.stream_id, Some("stream456".to_string()));
        assert_eq!(session.protocol, Some("rtmp".to_string()));
        assert_eq!(session.metadata.get("ip"), Some(&"192.168.1.1".to_string()));
    }

    #[test]
    fn test_session_expiration() {
        let mut session = SessionData::new("user123".to_string());
        
        // 刚创建的会话不应该过期
        assert!(!session.is_expired(3600));
        
        // 修改最后活跃时间为 2 小时前
        session.last_active = Utc::now() - chrono::Duration::hours(2);
        
        // 现在应该过期了（TTL 1 小时）
        assert!(session.is_expired(3600));
    }
}
