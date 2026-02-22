use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::SessionData;

/// 会话存储 trait
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// 保存会话
    async fn save(&self, session_id: &str, data: SessionData) -> Result<()>;
    
    /// 加载会话
    async fn load(&self, session_id: &str) -> Result<Option<SessionData>>;
    
    /// 删除会话
    async fn delete(&self, session_id: &str) -> Result<()>;
    
    /// 检查会话是否存在
    async fn exists(&self, session_id: &str) -> Result<bool>;
    
    /// 清理过期会话
    async fn cleanup_expired(&self, ttl_seconds: i64) -> Result<u64>;
}

/// 内存会话存储（用于开发和测试）
pub struct MemorySessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionData>>>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn save(&self, session_id: &str, data: SessionData) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.to_string(), data);
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Option<SessionData>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    async fn delete(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }

    async fn exists(&self, session_id: &str) -> Result<bool> {
        let sessions = self.sessions.read().await;
        Ok(sessions.contains_key(session_id))
    }

    async fn cleanup_expired(&self, ttl_seconds: i64) -> Result<u64> {
        let mut sessions = self.sessions.write().await;
        let before_count = sessions.len();
        
        sessions.retain(|_, session| !session.is_expired(ttl_seconds));
        
        let removed = (before_count - sessions.len()) as u64;
        Ok(removed)
    }
}

/// Redis 会话存储
#[cfg(feature = "redis-session")]
pub struct RedisSessionStore {
    client: redis::Client,
    ttl: Duration,
}

#[cfg(feature = "redis-session")]
impl RedisSessionStore {
    pub fn new(redis_url: &str, ttl: Duration) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self { client, ttl })
    }
}

#[cfg(feature = "redis-session")]
#[async_trait]
impl SessionStore for RedisSessionStore {
    async fn save(&self, session_id: &str, data: SessionData) -> Result<()> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let json = serde_json::to_string(&data)?;
        
        conn.set_ex(
            format!("session:{}", session_id),
            json,
            self.ttl.as_secs() as u64,
        )
        .await?;
        
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Option<SessionData>> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let result: Option<String> = conn.get(format!("session:{}", session_id)).await?;
        
        match result {
            Some(json) => {
                let data = serde_json::from_str(&json)?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    async fn delete(&self, session_id: &str) -> Result<()> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        conn.del(format!("session:{}", session_id)).await?;
        
        Ok(())
    }

    async fn exists(&self, session_id: &str) -> Result<bool> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let exists: bool = conn.exists(format!("session:{}", session_id)).await?;
        
        Ok(exists)
    }

    async fn cleanup_expired(&self, _ttl_seconds: i64) -> Result<u64> {
        // Redis 自动处理过期，无需手动清理
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_store() {
        let store = MemorySessionStore::new();
        let session = SessionData::new("user123".to_string());
        let session_id = session.session_id.clone();

        // 保存会话
        store.save(&session_id, session.clone()).await.unwrap();

        // 加载会话
        let loaded = store.load(&session_id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().user_id, "user123");

        // 检查存在
        assert!(store.exists(&session_id).await.unwrap());

        // 删除会话
        store.delete(&session_id).await.unwrap();
        assert!(!store.exists(&session_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let store = MemorySessionStore::new();
        
        // 创建一个过期的会话
        let mut session = SessionData::new("user123".to_string());
        session.last_active = chrono::Utc::now() - chrono::Duration::hours(2);
        let session_id = session.session_id.clone();
        
        store.save(&session_id, session).await.unwrap();

        // 清理过期会话（TTL 1 小时）
        let removed = store.cleanup_expired(3600).await.unwrap();
        assert_eq!(removed, 1);
    }
}
