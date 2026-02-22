use thiserror::Error;

/// 设备管理错误类型
#[derive(Error, Debug)]
pub enum DeviceError {
    /// 设备未找到
    #[error("Device not found: {0}")]
    NotFound(String),

    /// 设备已存在
    #[error("Device already exists: {0}")]
    AlreadyExists(String),

    /// 设备状态无效
    #[error("Invalid device status: {0}")]
    InvalidStatus(String),

    /// 设备分组未找到
    #[error("Device group not found: {0}")]
    GroupNotFound(String),

    /// 设备分组已存在
    #[error("Device group already exists: {0}")]
    GroupAlreadyExists(String),

    /// 数据库错误
    #[error("Database error: {0}")]
    DatabaseError(#[from] sea_orm::DbErr),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// 验证错误
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// 权限错误
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// 内部错误
    #[error("Internal error: {0}")]
    InternalError(String),

    /// 其他错误
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// 设备管理结果类型
pub type Result<T> = std::result::Result<T, DeviceError>;

impl DeviceError {
    /// 创建验证错误
    pub fn validation(msg: impl Into<String>) -> Self {
        DeviceError::ValidationError(msg.into())
    }

    /// 创建内部错误
    pub fn internal(msg: impl Into<String>) -> Self {
        DeviceError::InternalError(msg.into())
    }

    /// 创建权限错误
    pub fn permission_denied(msg: impl Into<String>) -> Self {
        DeviceError::PermissionDenied(msg.into())
    }
}
