use thiserror::Error;

#[derive(Error, Debug)]
pub enum VideoError {
    #[error("Stream not found: {0}")]
    StreamNotFound(String),
    
    #[error("Stream already exists: {0}")]
    StreamAlreadyExists(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Queue full")]
    QueueFull,
    
    #[error("Invalid packet")]
    InvalidPacket,
    
    #[error("Timeout")]
    Timeout,
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, VideoError>;
