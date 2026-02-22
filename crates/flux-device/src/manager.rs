use crate::{
    Device, DeviceError, DeviceFilter, DeviceGroup, DeviceGroupManager, DeviceMetrics,
    DeviceMonitor, DeviceRegistry, DeviceStatus, Result,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// 设备管理器
/// 
/// 统一的设备管理入口，整合了设备注册、监控和分组功能
pub struct DeviceManager {
    /// 设备注册表
    registry: Arc<DeviceRegistry>,
    
    /// 设备监控器
    monitor: Arc<DeviceMonitor>,
    
    /// 设备分组管理器
    group_manager: Arc<DeviceGroupManager>,
}

impl DeviceManager {
    /// 创建新的设备管理器
    /// 
    /// # 参数
    /// * `db` - 数据库连接
    /// * `heartbeat_interval` - 心跳间隔（秒）
    /// * `timeout` - 超时时间（秒）
    pub fn new(db: Arc<DatabaseConnection>, heartbeat_interval: u64, timeout: u64) -> Self {
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        let monitor = Arc::new(DeviceMonitor::new(
            registry.clone(),
            heartbeat_interval,
            timeout,
        ));
        let group_manager = Arc::new(DeviceGroupManager::new(db, registry.clone()));

        info!("Device manager created");

        Self {
            registry,
            monitor,
            group_manager,
        }
    }

    /// 启动设备管理器
    /// 
    /// 启动后台监控任务
    pub async fn start(&self) {
        self.monitor.start().await;
        info!("Device manager started");
    }

    /// 停止设备管理器
    pub async fn stop(&self) {
        self.monitor.stop().await;
        info!("Device manager stopped");
    }

    // ========== 设备管理 ==========

    /// 注册设备
    pub async fn register_device(&self, device: Device) -> Result<Device> {
        self.registry.register(device).await
    }

    /// 获取设备信息
    pub async fn get_device(&self, device_id: &str) -> Result<Option<Device>> {
        self.registry.get(device_id).await
    }

    /// 列出设备
    pub async fn list_devices(&self, filter: DeviceFilter) -> Result<Vec<Device>> {
        self.registry.list(filter).await
    }

    /// 更新设备信息
    pub async fn update_device(&self, device_id: &str, device: Device) -> Result<Device> {
        self.registry.update(device_id, device).await
    }

    /// 删除设备
    pub async fn delete_device(&self, device_id: &str) -> Result<()> {
        self.registry.unregister(device_id).await
    }

    /// 统计设备数量
    pub async fn count_devices(&self, filter: DeviceFilter) -> Result<u64> {
        self.registry.count(filter).await
    }

    // ========== 设备状态和监控 ==========

    /// 设备心跳
    pub async fn heartbeat(&self, device_id: &str) -> Result<()> {
        self.monitor.heartbeat(device_id).await
    }

    /// 获取设备状态
    pub async fn get_status(&self, device_id: &str) -> Result<DeviceStatus> {
        self.monitor.get_status(device_id).await
    }

    /// 设置设备状态
    pub async fn set_status(&self, device_id: &str, status: DeviceStatus) -> Result<()> {
        self.monitor.set_status(device_id, status).await
    }

    /// 检查设备是否在线
    pub async fn is_online(&self, device_id: &str) -> Result<bool> {
        self.monitor.is_online(device_id).await
    }

    /// 获取在线设备数量
    pub async fn online_count(&self) -> Result<u64> {
        self.monitor.online_count().await
    }

    /// 获取离线设备数量
    pub async fn offline_count(&self) -> Result<u64> {
        self.monitor.offline_count().await
    }

    /// 记录设备指标
    pub async fn record_metric(
        &self,
        device_id: &str,
        metric_name: String,
        metric_value: f64,
        unit: Option<String>,
    ) -> Result<()> {
        self.monitor.record_metric(device_id, metric_name, metric_value, unit).await
    }

    /// 获取设备指标
    pub async fn get_metrics(&self, device_id: &str) -> Result<Vec<DeviceMetrics>> {
        self.monitor.get_metrics(device_id).await
    }

    // ========== 设备分组 ==========

    /// 创建分组
    pub async fn create_group(&self, group: DeviceGroup) -> Result<DeviceGroup> {
        self.group_manager.create_group(group).await
    }

    /// 获取分组信息
    pub async fn get_group(&self, group_id: &str) -> Result<Option<DeviceGroup>> {
        self.group_manager.get_group(group_id).await
    }

    /// 更新分组
    pub async fn update_group(&self, group_id: &str, group: DeviceGroup) -> Result<DeviceGroup> {
        self.group_manager.update_group(group_id, group).await
    }

    /// 删除分组
    pub async fn delete_group(&self, group_id: &str) -> Result<()> {
        self.group_manager.delete_group(group_id).await
    }

    /// 列出所有分组
    pub async fn list_groups(&self) -> Result<Vec<DeviceGroup>> {
        self.group_manager.list_groups().await
    }

    /// 获取子分组
    pub async fn get_children(&self, parent_id: &str) -> Result<Vec<DeviceGroup>> {
        self.group_manager.get_children(parent_id).await
    }

    /// 添加设备到分组
    pub async fn add_to_group(&self, group_id: &str, device_id: &str) -> Result<()> {
        self.group_manager.add_device(group_id, device_id).await
    }

    /// 从分组移除设备
    pub async fn remove_from_group(&self, group_id: &str, device_id: &str) -> Result<()> {
        self.group_manager.remove_device(group_id, device_id).await
    }

    /// 获取分组下的设备
    pub async fn get_group_devices(&self, group_id: &str) -> Result<Vec<Device>> {
        self.group_manager.get_devices(group_id).await
    }

    /// 批量添加设备到分组
    pub async fn add_devices_batch(&self, group_id: &str, device_ids: &[String]) -> Result<usize> {
        self.group_manager.add_devices_batch(group_id, device_ids).await
    }

    /// 移动分组
    pub async fn move_group(&self, group_id: &str, new_parent_id: Option<String>) -> Result<()> {
        self.group_manager.move_group(group_id, new_parent_id).await
    }

    // ========== 辅助方法 ==========

    /// 获取设备注册表引用（用于高级操作）
    pub fn registry(&self) -> &Arc<DeviceRegistry> {
        &self.registry
    }

    /// 获取设备监控器引用（用于高级操作）
    pub fn monitor(&self) -> &Arc<DeviceMonitor> {
        &self.monitor
    }

    /// 获取设备分组管理器引用（用于高级操作）
    pub fn group_manager(&self) -> &Arc<DeviceGroupManager> {
        &self.group_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceType, Protocol};
    use sea_orm::{Database, ConnectionTrait, Statement};

    fn create_test_device(name: &str) -> Device {
        Device::new(
            name.to_string(),
            DeviceType::Sensor,
            Protocol::MQTT,
        )
    }

    async fn create_test_manager() -> DeviceManager {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        
        // 创建设备表
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                device_type TEXT NOT NULL,
                protocol TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'Inactive',
                product_id TEXT,
                secret TEXT,
                metadata TEXT,
                tags TEXT,
                group_id TEXT,
                location TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_seen TEXT
            )
            "#.to_string()
        )).await.unwrap();
        
        // 创建分组表
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE IF NOT EXISTS device_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                parent_id TEXT,
                path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#.to_string()
        )).await.unwrap();
        
        // 创建指标表
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE IF NOT EXISTS device_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_id TEXT NOT NULL,
                metric_name TEXT NOT NULL,
                metric_value REAL NOT NULL,
                unit TEXT,
                timestamp TEXT NOT NULL
            )
            "#.to_string()
        )).await.unwrap();
        
        DeviceManager::new(Arc::new(db), 30, 60)
    }

    #[tokio::test]
    async fn test_device_manager_lifecycle() {
        let manager = create_test_manager().await;

        // 启动管理器
        manager.start().await;

        // 注册设备
        let device = create_test_device("测试设备");
        let device_id = device.id.clone();
        let result = manager.register_device(device).await;
        assert!(result.is_ok());

        // 获取设备
        let device = manager.get_device(&device_id).await.unwrap();
        assert!(device.is_some());

        // 发送心跳
        manager.heartbeat(&device_id).await.unwrap();

        // 检查在线状态
        let is_online = manager.is_online(&device_id).await.unwrap();
        assert!(is_online);

        // 创建分组
        let group = DeviceGroup::new("测试分组".to_string(), None);
        let group_id = group.id.clone();
        manager.create_group(group).await.unwrap();

        // 添加设备到分组
        manager.add_to_group(&group_id, &device_id).await.unwrap();

        // 获取分组设备
        let devices = manager.get_group_devices(&group_id).await.unwrap();
        assert_eq!(devices.len(), 1);

        // 停止管理器
        manager.stop().await;
    }

    #[tokio::test]
    async fn test_device_statistics() {
        let manager = create_test_manager().await;

        // 注册多个设备
        for i in 0..5 {
            let device = create_test_device(&format!("设备{}", i));
            manager.register_device(device).await.unwrap();
        }

        // 统计设备数量
        let count = manager.count_devices(DeviceFilter::default()).await.unwrap();
        assert_eq!(count, 5);
    }
}
