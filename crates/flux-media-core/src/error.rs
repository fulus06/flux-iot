use thiserror::Error;

#[derive(Error, Debug)]
pub enum MediaError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Invalid stream ID: {0}")]
    InvalidStreamId(String),

    #[error("Snapshot not available: {0}")]
    SnapshotNotAvailable(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Decode error: {0}")]
    Decode(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, MediaError>;
