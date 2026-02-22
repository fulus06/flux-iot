use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

use super::{SessionData, SessionStore};

/// 会话管理器
pub struct SessionManager {
    store: Arc<dyn SessionStore>,
    ttl: Duration,
}

impl SessionManager {
    /// 创建新的会话管理器
    pub fn new(store: Arc<dyn SessionStore>, ttl: Duration) -> Self {
        let manager = Self { store, ttl };
        
        // 启动定期清理任务
        let store_clone = manager.store.clone();
        let ttl_secs = ttl.as_secs() as i64;
        tokio::spawn(async move {
            Self::cleanup_task(store_clone, ttl_secs).await;
        });
        
        manager
    }

    /// 创建新会话
    pub async fn create_session(&self, user_id: String) -> Result<SessionData> {
        let session = SessionData::new(user_id);
        self.store.save(&session.session_id, session.clone()).await?;
        
        info!(session_id = %session.session_id, user_id = %session.user_id, "Session created");
        Ok(session)
    }

    /// 获取会话
    pub async fn get_session(&self, session_id: &str) -> Result<Option<SessionData>> {
        let session = self.store.load(session_id).await?;
        
        if let Some(ref s) = session {
            // 检查是否过期
            if s.is_expired(self.ttl.as_secs() as i64) {
                self.store.delete(session_id).await?;
                debug!(session_id = session_id, "Session expired and removed");
                return Ok(None);
            }
        }
        
        Ok(session)
    }

    /// 更新会话
    pub async fn update_session(&self, session_id: &str, mut session: SessionData) -> Result<()> {
        session.touch();
        self.store.save(session_id, session).await?;
        debug!(session_id = session_id, "Session updated");
        Ok(())
    }

    /// 删除会话
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        self.store.delete(session_id).await?;
        info!(session_id = session_id, "Session deleted");
        Ok(())
    }

    /// 刷新会话（更新最后活跃时间）
    pub async fn refresh_session(&self, session_id: &str) -> Result<()> {
        if let Some(mut session) = self.store.load(session_id).await? {
            session.touch();
            self.store.save(session_id, session).await?;
            debug!(session_id = session_id, "Session refreshed");
        }
        Ok(())
    }

    /// 定期清理过期会话
    async fn cleanup_task(store: Arc<dyn SessionStore>, ttl_secs: i64) {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 每 5 分钟清理一次
        
        loop {
            interval.tick().await;
            
            match store.cleanup_expired(ttl_secs).await {
                Ok(removed) => {
                    if removed > 0 {
                        info!(removed = removed, "Cleaned up expired sessions");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to cleanup expired sessions");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::MemorySessionStore;

    #[tokio::test]
    async fn test_session_manager() {
        let store = Arc::new(MemorySessionStore::new());
        let manager = SessionManager::new(store, Duration::from_secs(3600));

        // 创建会话
        let session = manager.create_session("user123".to_string()).await.unwrap();
        let session_id = session.session_id.clone();

        // 获取会话
        let loaded = manager.get_session(&session_id).await.unwrap();
        assert!(loaded.is_some());

        // 刷新会话
        manager.refresh_session(&session_id).await.unwrap();

        // 删除会话
        manager.delete_session(&session_id).await.unwrap();
        let deleted = manager.get_session(&session_id).await.unwrap();
        assert!(deleted.is_none());
    }
}
