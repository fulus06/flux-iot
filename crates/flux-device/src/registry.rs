use crate::{Device, DeviceError, DeviceFilter, Result};
use crate::db::device;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, QuerySelect};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 设备注册表
/// 
/// 负责设备的注册、注销、查询和更新操作
/// 支持内存缓存以提高查询性能
pub struct DeviceRegistry {
    /// 数据库连接
    pub(crate) db: Arc<DatabaseConnection>,
    
    /// 内存缓存（设备ID -> 设备信息）
    cache: Arc<RwLock<HashMap<String, Device>>>,
    
    /// 是否启用缓存
    cache_enabled: bool,
}

impl DeviceRegistry {
    /// 创建新的设备注册表
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_enabled: true,
        }
    }

    /// 创建不启用缓存的设备注册表（用于测试）
    #[cfg(test)]
    pub fn new_without_cache(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_enabled: false,
        }
    }

    /// 注册设备
    /// 
    /// # 参数
    /// * `device` - 要注册的设备信息
    /// 
    /// # 返回
    /// 注册成功的设备信息
    /// 
    /// # 错误
    /// * `AlreadyExists` - 设备ID已存在
    /// * `ValidationError` - 设备信息验证失败
    /// * `DatabaseError` - 数据库操作失败
    pub async fn register(&self, mut device: Device) -> Result<Device> {
        // 验证设备信息
        self.validate_device(&device)?;

        // 检查设备是否已存在
        if self.exists(&device.id).await? {
            return Err(DeviceError::AlreadyExists(device.id.clone()));
        }

        // 设置创建时间和更新时间
        let now = chrono::Utc::now();
        device.created_at = now;
        device.updated_at = now;

        // 保存到数据库
        let active_model: device::ActiveModel = device.clone().into();
        device::Entity::insert(active_model)
            .exec(&*self.db)
            .await?;
        
        info!(
            device_id = %device.id,
            device_name = %device.name,
            device_type = ?device.device_type,
            "Device registered"
        );

        // 更新缓存
        if self.cache_enabled {
            let mut cache = self.cache.write().await;
            cache.insert(device.id.clone(), device.clone());
        }

        Ok(device)
    }

    /// 注销设备
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// 
    /// # 错误
    /// * `NotFound` - 设备不存在
    /// * `DatabaseError` - 数据库操作失败
    pub async fn unregister(&self, device_id: &str) -> Result<()> {
        // 检查设备是否存在
        if !self.exists(device_id).await? {
            return Err(DeviceError::NotFound(device_id.to_string()));
        }

        // 从数据库删除
        device::Entity::delete_by_id(device_id.to_string())
            .exec(&*self.db)
            .await?;

        info!(device_id = %device_id, "Device unregistered");

        // 从缓存删除
        if self.cache_enabled {
            let mut cache = self.cache.write().await;
            cache.remove(device_id);
        }

        Ok(())
    }

    /// 获取设备信息
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// 
    /// # 返回
    /// 设备信息，如果不存在则返回 None
    pub async fn get(&self, device_id: &str) -> Result<Option<Device>> {
        // 先从缓存查询
        if self.cache_enabled {
            let cache = self.cache.read().await;
            if let Some(device) = cache.get(device_id) {
                debug!(device_id = %device_id, "Device found in cache");
                return Ok(Some(device.clone()));
            }
        }

        // 从数据库查询
        let model = device::Entity::find_by_id(device_id.to_string())
            .one(&*self.db)
            .await?;
        
        if let Some(model) = model {
            let device = Device::from(model);
            
            // 更新缓存
            if self.cache_enabled {
                let mut cache = self.cache.write().await;
                cache.insert(device_id.to_string(), device.clone());
            }
            
            debug!(device_id = %device_id, "Device found in database");
            Ok(Some(device))
        } else {
            debug!(device_id = %device_id, "Device not found");
            Ok(None)
        }
    }

    /// 更新设备信息
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// * `device` - 新的设备信息
    /// 
    /// # 返回
    /// 更新后的设备信息
    /// 
    /// # 错误
    /// * `NotFound` - 设备不存在
    /// * `ValidationError` - 设备信息验证失败
    /// * `DatabaseError` - 数据库操作失败
    pub async fn update(&self, device_id: &str, mut device: Device) -> Result<Device> {
        // 检查设备是否存在
        if !self.exists(device_id).await? {
            return Err(DeviceError::NotFound(device_id.to_string()));
        }

        // 验证设备信息
        self.validate_device(&device)?;

        // 更新时间
        device.updated_at = chrono::Utc::now();

        // 更新到数据库
        let active_model: device::ActiveModel = device.clone().into();
        active_model.update(&*self.db).await?;

        info!(device_id = %device_id, "Device updated");

        // 更新缓存
        if self.cache_enabled {
            let mut cache = self.cache.write().await;
            cache.insert(device_id.to_string(), device.clone());
        }

        Ok(device)
    }

    /// 列出设备
    /// 
    /// # 参数
    /// * `filter` - 过滤条件
    /// 
    /// # 返回
    /// 符合条件的设备列表
    pub async fn list(&self, filter: DeviceFilter) -> Result<Vec<Device>> {
        use sea_orm::{QueryOrder, PaginatorTrait};
        
        // 构建查询
        let mut query = device::Entity::find();
        
        // 应用过滤条件
        if let Some(device_type) = &filter.device_type {
            query = query.filter(device::Column::DeviceType.eq(device_type.as_str()));
        }
        if let Some(protocol) = &filter.protocol {
            query = query.filter(device::Column::Protocol.eq(protocol.as_str()));
        }
        if let Some(status) = &filter.status {
            query = query.filter(device::Column::Status.eq(status.as_str()));
        }
        if let Some(group_id) = &filter.group_id {
            query = query.filter(device::Column::GroupId.eq(group_id));
        }
        
        // 排序
        query = query.order_by_desc(device::Column::CreatedAt);
        
        // 分页
        let page = filter.page.unwrap_or(1);
        let page_size = filter.page_size.unwrap_or(20);
        
        let models = query
            .paginate(&*self.db, page_size)
            .fetch_page(page - 1)
            .await?;
        
        // 转换为 Device
        let devices: Vec<Device> = models.into_iter().map(|m| Device::from(m)).collect();
        
        debug!(count = devices.len(), "Devices listed from database");
        Ok(devices)
    }

    /// 检查设备是否存在
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// 
    /// # 返回
    /// 如果设备存在返回 true，否则返回 false
    pub async fn exists(&self, device_id: &str) -> Result<bool> {
        // 先查缓存
        if self.cache_enabled {
            let cache = self.cache.read().await;
            if cache.contains_key(device_id) {
                return Ok(true);
            }
        }

        // 从数据库查询
        use sea_orm::PaginatorTrait;
        let count = device::Entity::find_by_id(device_id.to_string())
            .count(&*self.db)
            .await?;
        
        Ok(count > 0)
    }

    /// 统计设备数量
    /// 
    /// # 参数
    /// * `filter` - 过滤条件
    /// 
    /// # 返回
    /// 符合条件的设备数量
    pub async fn count(&self, filter: DeviceFilter) -> Result<u64> {
        use sea_orm::PaginatorTrait;
        
        // 构建查询
        let mut query = device::Entity::find();
        
        // 应用过滤条件
        if let Some(device_type) = &filter.device_type {
            query = query.filter(device::Column::DeviceType.eq(device_type.as_str()));
        }
        if let Some(protocol) = &filter.protocol {
            query = query.filter(device::Column::Protocol.eq(protocol.as_str()));
        }
        if let Some(status) = &filter.status {
            query = query.filter(device::Column::Status.eq(status.as_str()));
        }
        if let Some(group_id) = &filter.group_id {
            query = query.filter(device::Column::GroupId.eq(group_id));
        }
        
        let count = query.count(&*self.db).await?;
        Ok(count)
    }

    /// 清空缓存
    pub async fn clear_cache(&self) {
        if self.cache_enabled {
            let mut cache = self.cache.write().await;
            cache.clear();
            info!("Device cache cleared");
        }
    }

    /// 预热缓存
    /// 
    /// 从数据库加载所有设备到缓存
    pub async fn warm_cache(&self) -> Result<()> {
        if !self.cache_enabled {
            return Ok(());
        }

        // 从数据库加载所有设备（限制数量避免内存溢出）
        use sea_orm::QuerySelect;
        let models = device::Entity::find()
            .limit(10000) // 最多加载10000个设备
            .all(&*self.db)
            .await?;
        
        let mut cache = self.cache.write().await;
        for model in models {
            let device = Device::from(model);
            cache.insert(device.id.clone(), device);
        }
        
        info!(count = cache.len(), "Device cache warmed");
        Ok(())
    }

    // ========== 私有辅助方法 ==========

    /// 验证设备信息
    fn validate_device(&self, device: &Device) -> Result<()> {
        // 验证设备ID
        if device.id.is_empty() {
            return Err(DeviceError::validation("Device ID cannot be empty"));
        }

        // 验证设备名称
        if device.name.is_empty() {
            return Err(DeviceError::validation("Device name cannot be empty"));
        }

        // 验证设备名称长度
        if device.name.len() > 255 {
            return Err(DeviceError::validation("Device name too long (max 255 characters)"));
        }

        Ok(())
    }

    /// 应用过滤条件
    fn apply_filter(&self, mut devices: Vec<Device>, filter: &DeviceFilter) -> Vec<Device> {
        // 按设备类型过滤
        if let Some(ref device_type) = filter.device_type {
            devices.retain(|d| &d.device_type == device_type);
        }

        // 按协议过滤
        if let Some(ref protocol) = filter.protocol {
            devices.retain(|d| &d.protocol == protocol);
        }

        // 按状态过滤
        if let Some(ref status) = filter.status {
            devices.retain(|d| &d.status == status);
        }

        // 按分组过滤
        if let Some(ref group_id) = filter.group_id {
            devices.retain(|d| d.group_id.as_ref() == Some(group_id));
        }

        // 按标签过滤（包含任一标签）
        if let Some(ref tags) = filter.tags {
            devices.retain(|d| {
                tags.iter().any(|tag| d.tags.contains(tag))
            });
        }

        // 按搜索关键词过滤
        if let Some(ref search) = filter.search {
            let search_lower = search.to_lowercase();
            devices.retain(|d| {
                d.id.to_lowercase().contains(&search_lower)
                    || d.name.to_lowercase().contains(&search_lower)
            });
        }

        devices
    }

    /// 应用分页
    fn apply_pagination(&self, devices: Vec<Device>, filter: &DeviceFilter) -> Vec<Device> {
        let page = filter.page.unwrap_or(1);
        let page_size = filter.page_size.unwrap_or(20);

        let start = ((page - 1) * page_size) as usize;
        let end = (start + page_size as usize).min(devices.len());

        if start >= devices.len() {
            return Vec::new();
        }

        devices[start..end].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceType, Protocol, DeviceStatus};
    use sea_orm::Database;

    fn create_test_device(_id: &str, name: &str) -> Device {
        Device::new(
            name.to_string(),
            DeviceType::Sensor,
            Protocol::MQTT,
        )
    }

    async fn create_test_registry() -> DeviceRegistry {
        use sea_orm::{ConnectionTrait, Statement};
        
        let db = Database::connect("sqlite::memory:").await.unwrap();
        
        // 创建表结构
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
        
        DeviceRegistry::new_without_cache(Arc::new(db))
    }

    #[tokio::test]
    async fn test_register_device() {
        let registry = create_test_registry().await;

        let device = create_test_device("test_001", "测试设备");
        let result = registry.register(device.clone()).await;

        assert!(result.is_ok());
        let registered = result.unwrap();
        assert_eq!(registered.name, "测试设备");
    }

    #[tokio::test]
    async fn test_register_duplicate() {
        let registry = create_test_registry().await;

        let device = create_test_device("test_001", "测试设备");
        registry.register(device.clone()).await.unwrap();

        // 尝试注册相同ID的设备
        let result = registry.register(device).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DeviceError::AlreadyExists(_)));
    }

    #[tokio::test]
    async fn test_get_device() {
        let registry = create_test_registry().await;

        let device = create_test_device("test_001", "测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();

        let result = registry.get(&device_id).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "测试设备");
    }

    #[tokio::test]
    async fn test_update_device() {
        let registry = create_test_registry().await;

        let mut device = create_test_device("test_001", "测试设备");
        let device_id = device.id.clone();
        registry.register(device.clone()).await.unwrap();

        // 更新设备名称
        device.name = "更新后的设备".to_string();
        let result = registry.update(&device_id, device).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.name, "更新后的设备");
    }

    #[tokio::test]
    async fn test_unregister_device() {
        let registry = create_test_registry().await;

        let device = create_test_device("test_001", "测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();

        let result = registry.unregister(&device_id).await;
        assert!(result.is_ok());

        // 验证设备已删除
        let exists = registry.exists(&device_id).await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_list_devices() {
        let registry = create_test_registry().await;

        // 注册多个设备
        for i in 0..5 {
            let device = create_test_device(&format!("test_{:03}", i), &format!("设备{}", i));
            registry.register(device).await.unwrap();
        }

        let filter = DeviceFilter::default();
        let devices = registry.list(filter).await.unwrap();
        assert_eq!(devices.len(), 5);
    }

    #[tokio::test]
    async fn test_filter_by_type() {
        let registry = create_test_registry().await;

        // 注册不同类型的设备
        let mut sensor = create_test_device("sensor_001", "传感器");
        sensor.device_type = DeviceType::Sensor;
        registry.register(sensor).await.unwrap();

        let mut camera = create_test_device("camera_001", "摄像头");
        camera.device_type = DeviceType::Camera;
        registry.register(camera).await.unwrap();

        // 按类型过滤
        let filter = DeviceFilter {
            device_type: Some(DeviceType::Sensor),
            ..Default::default()
        };
        let devices = registry.list(filter).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_type, DeviceType::Sensor);
    }

    #[tokio::test]
    async fn test_pagination() {
        let registry = create_test_registry().await;

        // 注册10个设备
        for i in 0..10 {
            let device = create_test_device(&format!("test_{:03}", i), &format!("设备{}", i));
            registry.register(device).await.unwrap();
        }

        // 第一页，每页3个
        let filter = DeviceFilter {
            page: Some(1),
            page_size: Some(3),
            ..Default::default()
        };
        let devices = registry.list(filter).await.unwrap();
        assert_eq!(devices.len(), 3);

        // 第二页
        let filter = DeviceFilter {
            page: Some(2),
            page_size: Some(3),
            ..Default::default()
        };
        let devices = registry.list(filter).await.unwrap();
        assert_eq!(devices.len(), 3);
    }

    #[tokio::test]
    async fn test_count_devices() {
        let registry = create_test_registry().await;

        // 注册5个设备
        for i in 0..5 {
            let device = create_test_device(&format!("test_{:03}", i), &format!("设备{}", i));
            registry.register(device).await.unwrap();
        }

        let filter = DeviceFilter::default();
        let count = registry.count(filter).await.unwrap();
        assert_eq!(count, 5);
    }
}
