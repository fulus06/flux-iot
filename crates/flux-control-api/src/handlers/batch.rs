use crate::error::ApiError;
use axum::{extract::State, Json};
use flux_control::{BatchCommand, BatchExecutor, CommandType};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 批量控制 API 状态
#[derive(Clone)]
pub struct BatchAppState {
    pub batch_executor: Arc<BatchExecutor>,
}

/// 批量指令请求
#[derive(Debug, Deserialize)]
pub struct BatchCommandRequest {
    pub name: Option<String>,
    pub device_ids: Vec<String>,
    pub command_type: CommandType,
    pub params: Option<serde_json::Value>,
    pub concurrency: Option<usize>,
    pub continue_on_error: Option<bool>,
    pub timeout_seconds: Option<u64>,
}

/// 批量指令响应
#[derive(Debug, Serialize)]
pub struct BatchCommandResponse {
    pub batch_id: String,
    pub total_devices: usize,
    pub status: String,
}

/// 批量结果响应
#[derive(Debug, Serialize)]
pub struct BatchResultResponse {
    pub batch_id: String,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub timeout: usize,
    pub success_rate: f64,
    pub duration_ms: Option<i64>,
    pub results: Vec<serde_json::Value>,
}

/// 执行批量指令
pub async fn execute_batch_command(
    State(state): State<BatchAppState>,
    Json(req): Json<BatchCommandRequest>,
) -> Result<Json<BatchResultResponse>, ApiError> {
    // 验证设备列表
    if req.device_ids.is_empty() {
        return Err(ApiError::BadRequest(
            "Device list cannot be empty".to_string(),
        ));
    }

    // 创建批量指令
    let mut batch = BatchCommand::new(req.device_ids.clone(), req.command_type);

    if let Some(name) = req.name {
        batch = batch.with_name(name);
    }

    if let Some(params) = req.params {
        batch = batch.with_params(params);
    }

    if let Some(concurrency) = req.concurrency {
        batch = batch.with_concurrency(concurrency);
    }

    if let Some(continue_on_error) = req.continue_on_error {
        batch = batch.with_continue_on_error(continue_on_error);
    }

    if let Some(timeout) = req.timeout_seconds {
        batch = batch.with_timeout(timeout);
    }

    // 执行批量指令
    let result = state.batch_executor.execute(batch).await?;

    // 转换结果
    let results: Vec<serde_json::Value> = result
        .results
        .iter()
        .map(|r| {
            serde_json::json!({
                "device_id": r.device_id,
                "command_id": r.command_id,
                "status": format!("{:?}", r.status),
                "result": r.result,
                "error": r.error,
                "duration_ms": r.duration_ms,
            })
        })
        .collect();

    let success_rate = result.success_rate();

    Ok(Json(BatchResultResponse {
        batch_id: result.batch_id.clone(),
        total: result.total,
        success: result.success,
        failed: result.failed,
        timeout: result.timeout,
        success_rate,
        duration_ms: result.duration_ms,
        results,
    }))
}
