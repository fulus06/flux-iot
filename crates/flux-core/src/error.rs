use thiserror::Error;

/// FLUX Core 统一错误类型
#[derive(Error, Debug)]
pub enum FluxError {
    #[error("EventBus error: {0}")]
    EventBus(String),

    #[error("Channel send error: {0}")]
    ChannelSend(String),

    #[error("Channel receive error: {0}")]
    ChannelReceive(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Script error: {0}")]
    Script(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Wasm error: {0}")]
    Wasm(String),

    #[error("MQTT error: {0}")]
    Mqtt(String),

    #[error("TLS error: {0}")]
    Tls(String),
}

/// Result 类型别名
pub type Result<T> = std::result::Result<T, FluxError>;

impl From<anyhow::Error> for FluxError {
    fn from(err: anyhow::Error) -> Self {
        FluxError::Internal(err.to_string())
    }
}

impl<T> From<tokio::sync::broadcast::error::SendError<T>> for FluxError {
    fn from(err: tokio::sync::broadcast::error::SendError<T>) -> Self {
        FluxError::ChannelSend(err.to_string())
    }
}

impl From<tokio::sync::broadcast::error::RecvError> for FluxError {
    fn from(err: tokio::sync::broadcast::error::RecvError) -> Self {
        FluxError::ChannelReceive(err.to_string())
    }
}
