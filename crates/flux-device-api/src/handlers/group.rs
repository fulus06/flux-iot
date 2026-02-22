use crate::{error::Result, models::*, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use flux_device::DeviceGroup;
use tracing::{debug, info};

/// 创建设备分组
pub async fn create_group(
    State(state): State<AppState>,
    Json(req): Json<CreateGroupRequest>,
) -> Result<(StatusCode, Json<GroupResponse>)> {
    info!(name = %req.name, "Creating device group");

    let group = DeviceGroup::new(req.name, req.parent_id);
    let group = state.device_manager.create_group(group).await?;
    
    Ok((StatusCode::CREATED, Json(GroupResponse::from(group))))
}

/// 获取设备分组
pub async fn get_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupResponse>> {
    debug!(group_id = %group_id, "Getting device group");

    let group = state.device_manager.get_group(&group_id).await?
        .ok_or_else(|| crate::error::ApiError::GroupNotFound(group_id))?;
    
    Ok(Json(GroupResponse::from(group)))
}

/// 列出所有分组
pub async fn list_groups(
    State(state): State<AppState>,
) -> Result<Json<Vec<GroupResponse>>> {
    debug!("Listing all device groups");

    let groups = state.device_manager.list_groups().await?;
    
    let response: Vec<GroupResponse> = groups.into_iter()
        .map(GroupResponse::from)
        .collect();
    
    Ok(Json(response))
}

/// 获取子分组
pub async fn get_children(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<Vec<GroupResponse>>> {
    debug!(group_id = %group_id, "Getting child groups");

    let children = state.device_manager.get_children(&group_id).await?;
    
    let response: Vec<GroupResponse> = children.into_iter()
        .map(GroupResponse::from)
        .collect();
    
    Ok(Json(response))
}

/// 删除设备分组
pub async fn delete_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<StatusCode> {
    info!(group_id = %group_id, "Deleting device group");

    state.device_manager.delete_group(&group_id).await?;
    
    Ok(StatusCode::NO_CONTENT)
}

/// 添加设备到分组
pub async fn add_device_to_group(
    State(state): State<AppState>,
    Path((group_id, device_id)): Path<(String, String)>,
) -> Result<StatusCode> {
    info!(
        group_id = %group_id,
        device_id = %device_id,
        "Adding device to group"
    );

    state.device_manager.group_manager().add_device(&group_id, &device_id).await?;
    
    Ok(StatusCode::OK)
}

/// 从分组移除设备
pub async fn remove_device_from_group(
    State(state): State<AppState>,
    Path((group_id, device_id)): Path<(String, String)>,
) -> Result<StatusCode> {
    info!(
        group_id = %group_id,
        device_id = %device_id,
        "Removing device from group"
    );

    state.device_manager.group_manager().remove_device(&group_id, &device_id).await?;
    
    Ok(StatusCode::OK)
}

/// 获取分组中的设备
pub async fn get_group_devices(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<Vec<DeviceResponse>>> {
    debug!(group_id = %group_id, "Getting devices in group");

    let devices = state.device_manager.get_group_devices(&group_id).await?;
    
    let response: Vec<DeviceResponse> = devices.into_iter()
        .map(DeviceResponse::from)
        .collect();
    
    Ok(Json(response))
}
