use crate::error::ApiError;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use flux_control::{CommandExecutor, CommandType, DeviceCommand};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub executor: Arc<CommandExecutor>,
}

/// 发送指令请求
#[derive(Debug, Deserialize)]
pub struct SendCommandRequest {
    pub command_type: CommandType,
    pub timeout_seconds: Option<u64>,
}

/// 发送指令响应
#[derive(Debug, Serialize)]
pub struct SendCommandResponse {
    pub command_id: String,
    pub device_id: String,
    pub status: String,
}

/// 指令状态响应
#[derive(Debug, Serialize)]
pub struct CommandStatusResponse {
    pub command_id: String,
    pub device_id: String,
    pub command_type: String,
    pub status: String,
    pub created_at: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// 指令列表查询参数
#[derive(Debug, Deserialize)]
pub struct ListCommandsQuery {
    pub limit: Option<u64>,
}

/// 发送指令到设备
pub async fn send_command(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Json(req): Json<SendCommandRequest>,
) -> Result<Json<SendCommandResponse>, ApiError> {
    let mut command = DeviceCommand::new(device_id.clone(), req.command_type);
    
    if let Some(timeout) = req.timeout_seconds {
        command = command.with_timeout(std::time::Duration::from_secs(timeout));
    }

    let command_id = state.executor.submit(command.clone()).await?;

    // 异步执行指令
    let executor = state.executor.clone();
    tokio::spawn(async move {
        if let Err(e) = executor.execute(command).await {
            tracing::error!(error = %e, "Failed to execute command");
        }
    });

    Ok(Json(SendCommandResponse {
        command_id,
        device_id,
        status: "pending".to_string(),
    }))
}

/// 查询指令状态
pub async fn get_command_status(
    State(state): State<AppState>,
    Path(command_id): Path<String>,
) -> Result<Json<CommandStatusResponse>, ApiError> {
    let command = state
        .executor
        .get_command(&command_id)
        .await
        .ok_or_else(|| ApiError::NotFound(format!("Command {} not found", command_id)))?;

    Ok(Json(CommandStatusResponse {
        command_id: command.id,
        device_id: command.device_id,
        command_type: format!("{:?}", command.command_type),
        status: format!("{:?}", command.status),
        created_at: command.created_at.to_rfc3339(),
        result: command.result,
        error: command.error,
    }))
}

/// 取消指令
pub async fn cancel_command(
    State(state): State<AppState>,
    Path(command_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.executor.cancel(&command_id).await?;

    Ok(Json(serde_json::json!({
        "message": "Command cancelled successfully",
        "command_id": command_id,
    })))
}

/// 查询设备的指令历史
pub async fn list_device_commands(
    State(_state): State<AppState>,
    Path(_device_id): Path<String>,
    Query(_query): Query<ListCommandsQuery>,
) -> Result<Json<Vec<CommandStatusResponse>>, ApiError> {
    // TODO: 从数据库查询指令历史
    Ok(Json(vec![]))
}
