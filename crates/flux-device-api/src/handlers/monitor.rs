use crate::{error::Result, models::*, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::{debug, info};

/// 设备心跳
pub async fn heartbeat(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<StatusCode> {
    debug!(device_id = %device_id, "Device heartbeat");

    state.device_manager.heartbeat(&device_id).await?;
    
    Ok(StatusCode::OK)
}

/// 获取设备状态
pub async fn get_status(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!(device_id = %device_id, "Getting device status");

    let status = state.device_manager.get_status(&device_id).await?;
    
    Ok(Json(serde_json::json!({
        "device_id": device_id,
        "status": status,
    })))
}

/// 检查设备是否在线
pub async fn is_online(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!(device_id = %device_id, "Checking if device is online");

    let online = state.device_manager.is_online(&device_id).await?;
    
    Ok(Json(serde_json::json!({
        "device_id": device_id,
        "online": online,
    })))
}

/// 记录设备指标
pub async fn record_metric(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Json(req): Json<RecordMetricRequest>,
) -> Result<StatusCode> {
    info!(
        device_id = %device_id,
        metric_name = %req.metric_name,
        "Recording device metric"
    );

    state.device_manager.record_metric(
        &device_id,
        req.metric_name,
        req.metric_value,
        req.unit,
    ).await?;
    
    Ok(StatusCode::CREATED)
}

/// 获取设备指标
pub async fn get_metrics(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<Vec<MetricResponse>>> {
    debug!(device_id = %device_id, "Getting device metrics");

    let metrics = state.device_manager.get_metrics(&device_id).await?;
    
    let response: Vec<MetricResponse> = metrics.into_iter()
        .map(MetricResponse::from)
        .collect();
    
    Ok(Json(response))
}
