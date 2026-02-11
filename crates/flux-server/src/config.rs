use serde::Deserialize;

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
        }
    }
}
