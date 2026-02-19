use flux_video::gb28181::sip::{RegisterAuthMode, SipServerConfig};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub plugins: PluginConfig,
    #[serde(default)]
    pub eventbus: EventBusConfig,
    #[serde(default)]
    pub mqtt: MqttConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub gb28181: Gb28181Config,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Gb28181Config {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub backend: Gb28181Backend,
    #[serde(default)]
    pub remote: Gb28181RemoteConfig,
    #[serde(default)]
    pub sip: Gb28181SipConfig,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Gb28181Backend {
    Embedded,
    Remote,
}

impl Default for Gb28181Backend {
    fn default() -> Self {
        Self::Embedded
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Gb28181RemoteConfig {
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Gb28181SipConfig {
    pub bind_addr: Option<String>,
    pub sip_domain: Option<String>,
    pub sip_id: Option<String>,
    pub device_expires: Option<u32>,
    pub session_timeout: Option<i64>,
    #[serde(default)]
    pub auth: Gb28181SipAuthConfig,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Gb28181SipAuthConfig {
    #[serde(default)]
    pub mode: RegisterAuthModeConfig,
    pub global_password: Option<String>,
    #[serde(default)]
    pub per_device_passwords: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RegisterAuthModeConfig {
    None,
    Global,
    PerDevice,
    GlobalOrPerDevice,
}

impl Default for RegisterAuthModeConfig {
    fn default() -> Self {
        Self::None
    }
}

impl AppConfig {
    pub fn gb28181_sip_server_config(&self) -> SipServerConfig {
        let mut cfg = SipServerConfig::default();

        if let Some(v) = &self.gb28181.sip.bind_addr {
            cfg.bind_addr = v.clone();
        }
        if let Some(v) = &self.gb28181.sip.sip_domain {
            cfg.sip_domain = v.clone();
        }
        if let Some(v) = &self.gb28181.sip.sip_id {
            cfg.sip_id = v.clone();
        }
        if let Some(v) = self.gb28181.sip.device_expires {
            cfg.device_expires = v;
        }
        if let Some(v) = self.gb28181.sip.session_timeout {
            cfg.session_timeout = v;
        }

        cfg.auth_password = self.gb28181.sip.auth.global_password.clone();
        cfg.auth_mode = match self.gb28181.sip.auth.mode {
            RegisterAuthModeConfig::None => RegisterAuthMode::None,
            RegisterAuthModeConfig::Global => RegisterAuthMode::Global,
            RegisterAuthModeConfig::PerDevice => RegisterAuthMode::PerDevice,
            RegisterAuthModeConfig::GlobalOrPerDevice => RegisterAuthMode::GlobalOrPerDevice,
        };
        cfg.per_device_passwords = self.gb28181.sip.auth.per_device_passwords.clone();

        cfg
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PluginConfig {
    pub directory: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EventBusConfig {
    #[serde(default = "default_eventbus_capacity")]
    pub capacity: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MqttConfig {
    #[serde(default = "default_mqtt_port")]
    pub port: u16,
    #[serde(default = "default_mqtt_workers")]
    pub workers: usize,

    /// 启用 TLS/SSL
    #[serde(default)]
    pub enable_tls: bool,

    /// TLS 证书文件路径
    #[serde(default)]
    pub tls_cert_path: Option<String>,

    /// TLS 私钥文件路径
    #[serde(default)]
    pub tls_key_path: Option<String>,

    /// 启用客户端认证
    #[serde(default)]
    pub tls_client_auth: bool,

    /// CA 证书路径（用于客户端认证）
    #[serde(default)]
    pub tls_ca_cert_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

// 默认值函数
fn default_eventbus_capacity() -> usize {
    1024
}

fn default_mqtt_port() -> u16 {
    1883
}

fn default_mqtt_workers() -> usize {
    2
}

fn default_log_level() -> String {
    "info".to_string()
}

// Default trait 实现
impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            capacity: default_eventbus_capacity(),
        }
    }
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            port: default_mqtt_port(),
            workers: default_mqtt_workers(),
            enable_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            tls_client_auth: false,
            tls_ca_cert_path: None,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            database: DatabaseConfig {
                url: "sqlite::memory:".to_string(),
            },
            plugins: PluginConfig {
                directory: "plugins".to_string(),
            },
            eventbus: EventBusConfig::default(),
            mqtt: MqttConfig::default(),
            logging: LoggingConfig::default(),
            gb28181: Gb28181Config::default(),
        }
    }
}
