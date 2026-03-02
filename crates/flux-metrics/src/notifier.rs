use crate::alert::{Alert, AlertSeverity};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info};

/// 通知渠道接口
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn send(&self, alert: &Alert) -> Result<(), NotifierError>;
    fn name(&self) -> &str;
}

/// 通知错误
#[derive(Debug, thiserror::Error)]
pub enum NotifierError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Webhook 通知器
pub struct WebhookNotifier {
    url: String,
    client: reqwest::Client,
}

impl WebhookNotifier {
    pub fn new(url: String) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Notifier for WebhookNotifier {
    async fn send(&self, alert: &Alert) -> Result<(), NotifierError> {
        let payload = serde_json::to_string(alert)
            .map_err(|e| NotifierError::SerializationError(e.to_string()))?;

        self.client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await
            .map_err(|e| NotifierError::HttpError(e.to_string()))?;

        info!("Webhook notification sent to {}", self.url);
        Ok(())
    }

    fn name(&self) -> &str {
        "webhook"
    }
}

/// 钉钉通知器
pub struct DingTalkNotifier {
    webhook_url: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct DingTalkMessage {
    msgtype: String,
    markdown: DingTalkMarkdown,
}

#[derive(Serialize)]
struct DingTalkMarkdown {
    title: String,
    text: String,
}

impl DingTalkNotifier {
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            client: reqwest::Client::new(),
        }
    }

    fn format_message(&self, alert: &Alert) -> String {
        let severity_emoji = match alert.severity {
            AlertSeverity::Info => "ℹ️",
            AlertSeverity::Warning => "⚠️",
            AlertSeverity::Critical => "🔥",
        };

        let mut text = format!(
            "## {} 告警通知\n\n",
            severity_emoji
        );
        text.push_str(&format!("**告警名称**: {}\n\n", alert.name));
        text.push_str(&format!("**级别**: {:?}\n\n", alert.severity));
        text.push_str(&format!("**消息**: {}\n\n", alert.message));
        text.push_str(&format!("**状态**: {:?}\n\n", alert.state));
        text.push_str(&format!("**触发时间**: {}\n\n", alert.fired_at));

        if !alert.labels.is_empty() {
            text.push_str("**标签**:\n\n");
            for (k, v) in &alert.labels {
                text.push_str(&format!("- {}: {}\n", k, v));
            }
        }

        text
    }
}

#[async_trait]
impl Notifier for DingTalkNotifier {
    async fn send(&self, alert: &Alert) -> Result<(), NotifierError> {
        let message = DingTalkMessage {
            msgtype: "markdown".to_string(),
            markdown: DingTalkMarkdown {
                title: format!("告警: {}", alert.name),
                text: self.format_message(alert),
            },
        };

        let payload = serde_json::to_string(&message)
            .map_err(|e| NotifierError::SerializationError(e.to_string()))?;

        self.client
            .post(&self.webhook_url)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await
            .map_err(|e| NotifierError::HttpError(e.to_string()))?;

        info!("DingTalk notification sent");
        Ok(())
    }

    fn name(&self) -> &str {
        "dingtalk"
    }
}

/// 邮件通知器配置
#[derive(Debug, Clone)]
pub struct EmailConfig {
    /// SMTP 服务器地址
    pub smtp_server: String,
    /// SMTP 端口
    pub smtp_port: u16,
    /// 发件人邮箱
    pub from: String,
    /// SMTP 用户名
    pub username: String,
    /// SMTP 密码
    pub password: String,
    /// 是否使用 TLS
    pub use_tls: bool,
}

impl EmailConfig {
    pub fn new(smtp_server: String, from: String, username: String, password: String) -> Self {
        Self {
            smtp_server,
            smtp_port: 587, // 默认 STARTTLS 端口
            from,
            username,
            password,
            use_tls: true,
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.smtp_port = port;
        self
    }

    pub fn without_tls(mut self) -> Self {
        self.use_tls = false;
        self
    }
}

/// 邮件通知器（真实实现）
pub struct EmailNotifier {
    config: EmailConfig,
    to: Vec<String>,
}

impl EmailNotifier {
    pub fn new(config: EmailConfig, to: Vec<String>) -> Self {
        Self { config, to }
    }
}

#[async_trait]
impl Notifier for EmailNotifier {
    async fn send(&self, alert: &Alert) -> Result<(), NotifierError> {
        use lettre::{
            Message, SmtpTransport, Transport,
            transport::smtp::authentication::Credentials,
        };

        // 构建邮件内容
        let subject = format!("[{:?}] {}", alert.severity, alert.name);
        let mut body = format!(
            "告警详情:\n\n\
            名称: {}\n\
            级别: {:?}\n\
            状态: {:?}\n\
            消息: {}\n\
            触发时间: {}\n",
            alert.name,
            alert.severity,
            alert.state,
            alert.message,
            alert.fired_at
        );

        if !alert.labels.is_empty() {
            body.push_str("\n标签:\n");
            for (k, v) in &alert.labels {
                body.push_str(&format!("  {}: {}\n", k, v));
            }
        }

        // 发送给所有收件人
        for recipient in &self.to {
            // 构建邮件
            let email = Message::builder()
                .from(self.config.from.parse().map_err(|e| {
                    NotifierError::SendFailed(format!("Invalid from address: {}", e))
                })?)
                .to(recipient.parse().map_err(|e| {
                    NotifierError::SendFailed(format!("Invalid to address {}: {}", recipient, e))
                })?)
                .subject(&subject)
                .body(body.clone())
                .map_err(|e| NotifierError::SendFailed(format!("Failed to build email: {}", e)))?;

            // 创建 SMTP 传输
            let creds = Credentials::new(
                self.config.username.clone(),
                self.config.password.clone(),
            );

            let mailer = if self.config.use_tls {
                SmtpTransport::starttls_relay(&self.config.smtp_server)
                    .map_err(|e| NotifierError::SendFailed(format!("SMTP connection failed: {}", e)))?
                    .port(self.config.smtp_port)
                    .credentials(creds)
                    .build()
            } else {
                SmtpTransport::builder_dangerous(&self.config.smtp_server)
                    .port(self.config.smtp_port)
                    .credentials(creds)
                    .build()
            };

            // 发送邮件
            mailer.send(&email).map_err(|e| {
                NotifierError::SendFailed(format!("Failed to send email to {}: {}", recipient, e))
            })?;

            info!(
                to = %recipient,
                subject = %subject,
                "Email notification sent successfully"
            );
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "email"
    }
}

/// 通知管理器
pub struct NotificationManager {
    notifiers: Vec<Box<dyn Notifier>>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notifiers: Vec::new(),
        }
    }

    pub fn add_notifier(&mut self, notifier: Box<dyn Notifier>) {
        info!("Adding notifier: {}", notifier.name());
        self.notifiers.push(notifier);
    }

    pub async fn notify(&self, alert: &Alert) {
        for notifier in &self.notifiers {
            if let Err(e) = notifier.send(alert).await {
                error!("Failed to send notification via {}: {}", notifier.name(), e);
            }
        }
    }

    pub async fn notify_batch(&self, alerts: &[Alert]) {
        for alert in alerts {
            self.notify(alert).await;
        }
    }

    pub fn notifier_count(&self) -> usize {
        self.notifiers.len()
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::Alert;

    #[tokio::test]
    async fn test_notification_manager() {
        let mut manager = NotificationManager::new();

        let email_notifier = Box::new(EmailNotifier::new(
            "smtp.example.com".to_string(),
            "alert@example.com".to_string(),
            vec!["admin@example.com".to_string()],
        ));

        manager.add_notifier(email_notifier);
        assert_eq!(manager.notifier_count(), 1);

        let alert = Alert::new(
            "test_alert".to_string(),
            AlertSeverity::Warning,
            "Test message".to_string(),
            HashMap::new(),
        );

        manager.notify(&alert).await;
    }

    #[test]
    fn test_dingtalk_format() {
        let notifier = DingTalkNotifier::new("https://example.com/webhook".to_string());

        let mut labels = HashMap::new();
        labels.insert("host".to_string(), "server1".to_string());

        let alert = Alert::new(
            "high_cpu".to_string(),
            AlertSeverity::Critical,
            "CPU usage is 95%".to_string(),
            labels,
        );

        let message = notifier.format_message(&alert);
        assert!(message.contains("high_cpu"));
        assert!(message.contains("🔥"));
    }
}
