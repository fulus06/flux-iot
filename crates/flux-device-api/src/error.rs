use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;

/// API 错误类型
#[derive(Debug)]
pub enum ApiError {
    /// 设备未找到
    DeviceNotFound(String),
    /// 设备已存在
    DeviceAlreadyExists(String),
    /// 分组未找到
    GroupNotFound(String),
    /// 验证错误
    ValidationError(String),
    /// 数据库错误
    DatabaseError(String),
    /// 内部错误
    InternalError(String),
    /// 请求错误
    BadRequest(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::DeviceNotFound(id) => write!(f, "Device not found: {}", id),
            ApiError::DeviceAlreadyExists(id) => write!(f, "Device already exists: {}", id),
            ApiError::GroupNotFound(id) => write!(f, "Group not found: {}", id),
            ApiError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ApiError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            ApiError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            ApiError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::DeviceNotFound(ref msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::DeviceAlreadyExists(ref msg) => (StatusCode::CONFLICT, msg.clone()),
            ApiError::GroupNotFound(ref msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::ValidationError(ref msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::DatabaseError(ref msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::InternalError(ref msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::BadRequest(ref msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

// 从 flux_device::DeviceError 转换
impl From<flux_device::DeviceError> for ApiError {
    fn from(err: flux_device::DeviceError) -> Self {
        match err {
            flux_device::DeviceError::NotFound(id) => ApiError::DeviceNotFound(id),
            flux_device::DeviceError::AlreadyExists(id) => ApiError::DeviceAlreadyExists(id),
            flux_device::DeviceError::GroupNotFound(id) => ApiError::GroupNotFound(id),
            flux_device::DeviceError::GroupAlreadyExists(id) => ApiError::DeviceAlreadyExists(id),
            flux_device::DeviceError::InvalidStatus(msg) => ApiError::ValidationError(msg),
            flux_device::DeviceError::ValidationError(msg) => ApiError::ValidationError(msg),
            flux_device::DeviceError::DatabaseError(msg) => ApiError::DatabaseError(msg.to_string()),
            flux_device::DeviceError::SerializationError(msg) => ApiError::InternalError(msg.to_string()),
            flux_device::DeviceError::InternalError(msg) => ApiError::InternalError(msg),
            flux_device::DeviceError::PermissionDenied(msg) => ApiError::BadRequest(msg),
            flux_device::DeviceError::Other(err) => ApiError::InternalError(err.to_string()),
        }
    }
}

// 从 sea_orm::DbErr 转换
impl From<sea_orm::DbErr> for ApiError {
    fn from(err: sea_orm::DbErr) -> Self {
        ApiError::DatabaseError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;
