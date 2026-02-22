use crate::{error::Result, models::*, state::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use flux_device::Device;
use tracing::{debug, info};

/// 注册设备
pub async fn register_device(
    State(state): State<AppState>,
    Json(req): Json<RegisterDeviceRequest>,
) -> Result<(StatusCode, Json<DeviceResponse>)> {
    info!(name = %req.name, device_type = ?req.device_type, "Registering device");

    let mut device = Device::new(req.name, req.device_type, req.protocol);
    
    if let Some(product_id) = req.product_id {
        device.product_id = Some(product_id);
    }
    if let Some(secret) = req.secret {
        device.secret = Some(secret);
    }
    if let Some(metadata) = req.metadata {
        device.metadata = metadata;
    }
    if let Some(tags) = req.tags {
        device.tags = tags;
    }
    if let Some(group_id) = req.group_id {
        device.group_id = Some(group_id);
    }

    let device = state.device_manager.register_device(device).await?;
    
    Ok((StatusCode::CREATED, Json(DeviceResponse::from(device))))
}

/// 获取设备
pub async fn get_device(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<DeviceResponse>> {
    debug!(device_id = %device_id, "Getting device");

    let device = state.device_manager.get_device(&device_id).await?
        .ok_or_else(|| crate::error::ApiError::DeviceNotFound(device_id))?;
    
    Ok(Json(DeviceResponse::from(device)))
}

/// 列出设备
pub async fn list_devices(
    State(state): State<AppState>,
    Query(query): Query<ListDevicesQuery>,
) -> Result<Json<PaginatedResponse<DeviceResponse>>> {
    debug!("Listing devices with filter");

    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);
    
    let filter: flux_device::DeviceFilter = query.into();
    let devices = state.device_manager.list_devices(filter.clone()).await?;
    let total = state.device_manager.count_devices(filter).await?;
    
    let data: Vec<DeviceResponse> = devices.into_iter()
        .map(DeviceResponse::from)
        .collect();
    
    Ok(Json(PaginatedResponse {
        data,
        total,
        page,
        page_size,
    }))
}

/// 更新设备
pub async fn update_device(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Json(req): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceResponse>> {
    info!(device_id = %device_id, "Updating device");

    let mut device = state.device_manager.get_device(&device_id).await?
        .ok_or_else(|| crate::error::ApiError::DeviceNotFound(device_id.clone()))?;
    
    if let Some(name) = req.name {
        device.name = name;
    }
    if let Some(status) = req.status {
        device.status = status;
    }
    if let Some(metadata) = req.metadata {
        device.metadata = metadata;
    }
    if let Some(tags) = req.tags {
        device.tags = tags;
    }
    if let Some(group_id) = req.group_id {
        device.group_id = Some(group_id);
    }

    let device = state.device_manager.update_device(&device_id, device).await?;
    
    Ok(Json(DeviceResponse::from(device)))
}

/// 删除设备
pub async fn delete_device(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<StatusCode> {
    info!(device_id = %device_id, "Deleting device");

    state.device_manager.delete_device(&device_id).await?;
    
    Ok(StatusCode::NO_CONTENT)
}

/// 获取设备统计
pub async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<StatsResponse>> {
    debug!("Getting device statistics");

    let total_devices = state.device_manager.count_devices(Default::default()).await?;
    let online_devices = state.device_manager.online_count().await?;
    let offline_devices = state.device_manager.offline_count().await?;
    let total_groups = state.device_manager.group_manager().count().await?;
    
    Ok(Json(StatsResponse {
        total_devices,
        online_devices,
        offline_devices,
        total_groups,
    }))
}
