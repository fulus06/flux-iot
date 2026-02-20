use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 通知级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifyLevel {
    /// 信息
    Info,
    /// 警告
    Warning,
    /// 错误
    Error,
    /// 严重
    Critical,
}

/// 通知渠道
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotifyChannel {
    /// 邮件
    Email,
    /// Webhook
    Webhook,
    /// 钉钉
    DingTalk,
    /// 企业微信
    WeChat,
    /// Slack
    Slack,
    /// 短信
    SMS,
}

/// 通知消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyMessage {
    /// 标题
    pub title: String,
    
    /// 内容
    pub content: String,
    
    /// 级别
    pub level: NotifyLevel,
    
    /// 时间
    pub timestamp: DateTime<Utc>,
    
    /// 额外数据
    pub metadata: Option<serde_json::Value>,
}

impl NotifyMessage {
    pub fn new(title: impl Into<String>, content: impl Into<String>, level: NotifyLevel) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            level,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// 创建信息级别消息
    pub fn info(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(title, content, NotifyLevel::Info)
    }

    /// 创建警告级别消息
    pub fn warning(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(title, content, NotifyLevel::Warning)
    }

    /// 创建错误级别消息
    pub fn error(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(title, content, NotifyLevel::Error)
    }

    /// 创建严重级别消息
    pub fn critical(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(title, content, NotifyLevel::Critical)
    }
}
