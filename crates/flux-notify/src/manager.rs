use crate::message::{NotifyChannel, NotifyLevel, NotifyMessage};
use crate::notifier::Notifier;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// 通知管理器
pub struct NotifyManager {
    /// 通知器列表
    notifiers: Arc<RwLock<HashMap<NotifyChannel, Box<dyn Notifier>>>>,
    
    /// 最小通知级别
    min_level: NotifyLevel,
}

impl NotifyManager {
    pub fn new(min_level: NotifyLevel) -> Self {
        Self {
            notifiers: Arc::new(RwLock::new(HashMap::new())),
            min_level,
        }
    }

    /// 注册通知器
    pub async fn register(&self, channel: NotifyChannel, notifier: Box<dyn Notifier>) {
        let mut notifiers = self.notifiers.write().await;
        info!("Registered notifier: {}", notifier.name());
        notifiers.insert(channel, notifier);
    }

    /// 发送通知到指定渠道
    pub async fn send(&self, channel: NotifyChannel, message: &NotifyMessage) -> Result<()> {
        // 检查级别
        if !self.should_notify(&message.level) {
            return Ok(());
        }

        let notifiers = self.notifiers.read().await;
        
        if let Some(notifier) = notifiers.get(&channel) {
            if notifier.is_enabled() {
                match notifier.send(message).await {
                    Ok(result) => {
                        if result.success {
                            info!("Notification sent via {}: {}", notifier.name(), message.title);
                        } else {
                            error!("Notification failed via {}: {}", notifier.name(), result.message);
                        }
                    }
                    Err(e) => {
                        error!("Notification error via {}: {}", notifier.name(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// 发送通知到所有渠道
    pub async fn broadcast(&self, message: &NotifyMessage) -> Result<()> {
        // 检查级别
        if !self.should_notify(&message.level) {
            return Ok(());
        }

        let notifiers = self.notifiers.read().await;
        
        for notifier in notifiers.values() {
            if notifier.is_enabled() {
                match notifier.send(message).await {
                    Ok(result) => {
                        if result.success {
                            info!("Notification sent via {}: {}", notifier.name(), message.title);
                        } else {
                            error!("Notification failed via {}: {}", notifier.name(), result.message);
                        }
                    }
                    Err(e) => {
                        error!("Notification error via {}: {}", notifier.name(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// 检查是否应该发送通知
    fn should_notify(&self, level: &NotifyLevel) -> bool {
        let level_value = match level {
            NotifyLevel::Info => 0,
            NotifyLevel::Warning => 1,
            NotifyLevel::Error => 2,
            NotifyLevel::Critical => 3,
        };

        let min_value = match self.min_level {
            NotifyLevel::Info => 0,
            NotifyLevel::Warning => 1,
            NotifyLevel::Error => 2,
            NotifyLevel::Critical => 3,
        };

        level_value >= min_value
    }
}

impl Default for NotifyManager {
    fn default() -> Self {
        Self::new(NotifyLevel::Info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notify_manager() {
        let manager = NotifyManager::new(NotifyLevel::Warning);
        
        let message = NotifyMessage::warning(
            "Test Warning",
            "This is a test warning message"
        );
        
        // 应该通知（Warning >= Warning）
        assert!(manager.should_notify(&message.level));
        
        let info_message = NotifyMessage::info(
            "Test Info",
            "This is a test info message"
        );
        
        // 不应该通知（Info < Warning）
        assert!(!manager.should_notify(&info_message.level));
    }
}
