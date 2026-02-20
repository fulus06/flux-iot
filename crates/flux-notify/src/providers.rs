use crate::message::NotifyMessage;
use crate::notifier::{Notifier, NotifyResult};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ============================================================================
// 邮件通知
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
    pub to: Vec<String>,
}

pub struct EmailNotifier {
    config: EmailConfig,
    enabled: bool,
}

impl EmailNotifier {
    pub fn new(config: EmailConfig) -> Self {
        Self {
            config,
            enabled: true,
        }
    }
}

#[async_trait]
impl Notifier for EmailNotifier {
    async fn send(&self, message: &NotifyMessage) -> Result<NotifyResult> {
        use lettre::message::header::ContentType;
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::{Message, SmtpTransport, Transport};

        let email = Message::builder()
            .from(self.config.from.parse()?)
            .to(self.config.to[0].parse()?)
            .subject(&message.title)
            .header(ContentType::TEXT_PLAIN)
            .body(format!(
                "{}\n\nLevel: {:?}\nTime: {}",
                message.content, message.level, message.timestamp
            ))?;

        let creds = Credentials::new(
            self.config.username.clone(),
            self.config.password.clone(),
        );

        let mailer = SmtpTransport::relay(&self.config.smtp_host)?
            .credentials(creds)
            .port(self.config.smtp_port)
            .build();

        match mailer.send(&email) {
            Ok(_) => Ok(NotifyResult::success()),
            Err(e) => Ok(NotifyResult::failure(format!("Email send failed: {}", e))),
        }
    }

    fn name(&self) -> &str {
        "email"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ============================================================================
// Webhook 通知
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    pub method: String,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

pub struct WebhookNotifier {
    config: WebhookConfig,
    client: reqwest::Client,
    enabled: bool,
}

impl WebhookNotifier {
    pub fn new(config: WebhookConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            enabled: true,
        }
    }
}

#[async_trait]
impl Notifier for WebhookNotifier {
    async fn send(&self, message: &NotifyMessage) -> Result<NotifyResult> {
        let mut request = self.client.post(&self.config.url);

        if let Some(headers) = &self.config.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        let response = request.json(message).send().await?;

        if response.status().is_success() {
            Ok(NotifyResult::success())
        } else {
            Ok(NotifyResult::failure(format!(
                "Webhook failed with status: {}",
                response.status()
            )))
        }
    }

    fn name(&self) -> &str {
        "webhook"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ============================================================================
// 钉钉通知
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkConfig {
    pub webhook_url: String,
    pub secret: Option<String>,
}

pub struct DingTalkNotifier {
    config: DingTalkConfig,
    client: reqwest::Client,
    enabled: bool,
}

impl DingTalkNotifier {
    pub fn new(config: DingTalkConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            enabled: true,
        }
    }

    fn build_message(&self, message: &NotifyMessage) -> serde_json::Value {
        serde_json::json!({
            "msgtype": "markdown",
            "markdown": {
                "title": message.title,
                "text": format!(
                    "## {}\n\n{}\n\n**级别**: {:?}\n\n**时间**: {}",
                    message.title, message.content, message.level, message.timestamp
                )
            }
        })
    }
}

#[async_trait]
impl Notifier for DingTalkNotifier {
    async fn send(&self, message: &NotifyMessage) -> Result<NotifyResult> {
        let body = self.build_message(message);

        let response = self
            .client
            .post(&self.config.webhook_url)
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(NotifyResult::success())
        } else {
            Ok(NotifyResult::failure(format!(
                "DingTalk failed: {}",
                response.status()
            )))
        }
    }

    fn name(&self) -> &str {
        "dingtalk"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ============================================================================
// 企业微信通知
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeChatConfig {
    pub webhook_url: String,
}

pub struct WeChatNotifier {
    config: WeChatConfig,
    client: reqwest::Client,
    enabled: bool,
}

impl WeChatNotifier {
    pub fn new(config: WeChatConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            enabled: true,
        }
    }

    fn build_message(&self, message: &NotifyMessage) -> serde_json::Value {
        serde_json::json!({
            "msgtype": "markdown",
            "markdown": {
                "content": format!(
                    "## {}\n\n{}\n\n**级别**: {:?}\n**时间**: {}",
                    message.title, message.content, message.level, message.timestamp
                )
            }
        })
    }
}

#[async_trait]
impl Notifier for WeChatNotifier {
    async fn send(&self, message: &NotifyMessage) -> Result<NotifyResult> {
        let body = self.build_message(message);

        let response = self
            .client
            .post(&self.config.webhook_url)
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(NotifyResult::success())
        } else {
            Ok(NotifyResult::failure(format!(
                "WeChat failed: {}",
                response.status()
            )))
        }
    }

    fn name(&self) -> &str {
        "wechat"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ============================================================================
// Slack 通知
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub webhook_url: String,
}

pub struct SlackNotifier {
    config: SlackConfig,
    client: reqwest::Client,
    enabled: bool,
}

impl SlackNotifier {
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            enabled: true,
        }
    }

    fn build_message(&self, message: &NotifyMessage) -> serde_json::Value {
        let color = match message.level {
            crate::message::NotifyLevel::Info => "good",
            crate::message::NotifyLevel::Warning => "warning",
            crate::message::NotifyLevel::Error => "danger",
            crate::message::NotifyLevel::Critical => "danger",
        };

        serde_json::json!({
            "attachments": [{
                "color": color,
                "title": message.title,
                "text": message.content,
                "fields": [
                    {
                        "title": "Level",
                        "value": format!("{:?}", message.level),
                        "short": true
                    },
                    {
                        "title": "Time",
                        "value": message.timestamp.to_rfc3339(),
                        "short": true
                    }
                ]
            }]
        })
    }
}

#[async_trait]
impl Notifier for SlackNotifier {
    async fn send(&self, message: &NotifyMessage) -> Result<NotifyResult> {
        let body = self.build_message(message);

        let response = self
            .client
            .post(&self.config.webhook_url)
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(NotifyResult::success())
        } else {
            Ok(NotifyResult::failure(format!(
                "Slack failed: {}",
                response.status()
            )))
        }
    }

    fn name(&self) -> &str {
        "slack"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}
