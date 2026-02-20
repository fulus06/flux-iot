use crate::message::NotifyMessage;
use anyhow::Result;
use async_trait::async_trait;

/// 通知结果
#[derive(Debug, Clone)]
pub struct NotifyResult {
    pub success: bool,
    pub message: String,
}

impl NotifyResult {
    pub fn success() -> Self {
        Self {
            success: true,
            message: "Notification sent successfully".to_string(),
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}

/// 通知器 trait
#[async_trait]
pub trait Notifier: Send + Sync {
    /// 发送通知
    async fn send(&self, message: &NotifyMessage) -> Result<NotifyResult>;
    
    /// 通知器名称
    fn name(&self) -> &str;
    
    /// 是否启用
    fn is_enabled(&self) -> bool {
        true
    }
}
