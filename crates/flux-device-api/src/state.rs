use flux_device::DeviceManager;
use std::sync::Arc;

/// API 应用状态
#[derive(Clone)]
pub struct AppState {
    /// 设备管理器
    pub device_manager: Arc<DeviceManager>,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(device_manager: Arc<DeviceManager>) -> Self {
        Self { device_manager }
    }
}
