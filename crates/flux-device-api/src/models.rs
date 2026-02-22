use flux_device::{Device, DeviceFilter, DeviceGroup, DeviceMetrics, DeviceStatus, DeviceType, Protocol};
use serde::{Deserialize, Serialize};

/// 设备注册请求
#[derive(Debug, Deserialize)]
pub struct RegisterDeviceRequest {
    pub name: String,
    pub device_type: DeviceType,
    pub protocol: Protocol,
    pub product_id: Option<String>,
    pub secret: Option<String>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub tags: Option<Vec<String>>,
    pub group_id: Option<String>,
}

/// 设备更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub status: Option<DeviceStatus>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub tags: Option<Vec<String>>,
    pub group_id: Option<String>,
}

/// 设备查询请求
#[derive(Debug, Deserialize)]
pub struct ListDevicesQuery {
    pub device_type: Option<DeviceType>,
    pub protocol: Option<Protocol>,
    pub status: Option<DeviceStatus>,
    pub group_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub search: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

impl From<ListDevicesQuery> for DeviceFilter {
    fn from(query: ListDevicesQuery) -> Self {
        DeviceFilter {
            device_type: query.device_type,
            protocol: query.protocol,
            status: query.status,
            group_id: query.group_id,
            tags: query.tags,
            search: query.search,
            page: query.page,
            page_size: query.page_size,
        }
    }
}

/// 设备响应
#[derive(Debug, Serialize)]
pub struct DeviceResponse {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub protocol: Protocol,
    pub status: DeviceStatus,
    pub product_id: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub group_id: Option<String>,
    pub location: Option<flux_device::GeoLocation>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<Device> for DeviceResponse {
    fn from(device: Device) -> Self {
        Self {
            id: device.id,
            name: device.name,
            device_type: device.device_type,
            protocol: device.protocol,
            status: device.status,
            product_id: device.product_id,
            metadata: device.metadata,
            tags: device.tags,
            group_id: device.group_id,
            location: device.location,
            created_at: device.created_at,
            updated_at: device.updated_at,
            last_seen: device.last_seen,
        }
    }
}

/// 分组创建请求
#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<String>,
}

/// 分组响应
#[derive(Debug, Serialize)]
pub struct GroupResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<String>,
    pub path: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<DeviceGroup> for GroupResponse {
    fn from(group: DeviceGroup) -> Self {
        Self {
            id: group.id,
            name: group.name,
            description: group.description,
            parent_id: group.parent_id,
            path: group.path,
            created_at: group.created_at,
            updated_at: group.updated_at,
        }
    }
}

/// 指标记录请求
#[derive(Debug, Deserialize)]
pub struct RecordMetricRequest {
    pub metric_name: String,
    pub metric_value: f64,
    pub unit: Option<String>,
}

/// 指标响应
#[derive(Debug, Serialize)]
pub struct MetricResponse {
    pub id: i64,
    pub device_id: String,
    pub metric_name: String,
    pub metric_value: f64,
    pub unit: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl From<DeviceMetrics> for MetricResponse {
    fn from(metric: DeviceMetrics) -> Self {
        Self {
            id: metric.id,
            device_id: metric.device_id,
            metric_name: metric.metric_name,
            metric_value: metric.metric_value,
            unit: metric.unit,
            timestamp: metric.timestamp,
        }
    }
}

/// 分页响应
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

/// 统计响应
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_devices: u64,
    pub online_devices: u64,
    pub offline_devices: u64,
    pub total_groups: u64,
}
