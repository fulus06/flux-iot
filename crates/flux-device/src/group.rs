use crate::{Device, DeviceError, DeviceFilter, DeviceGroup, DeviceRegistry, Result};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 设备分组管理器
/// 
/// 负责设备分组的创建、管理和设备关联
pub struct DeviceGroupManager {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    
    /// 设备注册表
    registry: Arc<DeviceRegistry>,
    
    /// 分组缓存（分组ID -> 分组信息）
    groups: Arc<RwLock<HashMap<String, DeviceGroup>>>,
    
    /// 是否启用缓存
    cache_enabled: bool,
}

impl DeviceGroupManager {
    /// 创建新的设备分组管理器
    pub fn new(db: Arc<DatabaseConnection>, registry: Arc<DeviceRegistry>) -> Self {
        Self {
            db,
            registry,
            groups: Arc::new(RwLock::new(HashMap::new())),
            cache_enabled: true,
        }
    }

    /// 创建分组
    /// 
    /// # 参数
    /// * `group` - 分组信息
    /// 
    /// # 返回
    /// 创建成功的分组信息
    /// 
    /// # 错误
    /// * `AlreadyExists` - 分组ID已存在
    /// * `ValidationError` - 分组信息验证失败
    /// * `GroupNotFound` - 父分组不存在
    pub async fn create_group(&self, mut group: DeviceGroup) -> Result<DeviceGroup> {
        // 验证分组信息
        self.validate_group(&group)?;

        // 检查分组是否已存在
        if self.exists(&group.id).await? {
            return Err(DeviceError::GroupAlreadyExists(group.id.clone()));
        }

        // 如果有父分组，检查父分组是否存在
        if let Some(ref parent_id) = group.parent_id {
            if !self.exists(parent_id).await? {
                return Err(DeviceError::GroupNotFound(parent_id.clone()));
            }
            
            // 更新分组路径
            if let Some(parent) = self.get_group(parent_id).await? {
                group.path = format!("{}/{}", parent.path, group.id);
            }
        } else {
            // 根分组
            group.path = format!("/{}", group.id);
        }

        // 设置时间戳
        let now = chrono::Utc::now();
        group.created_at = now;
        group.updated_at = now;

        // 保存到数据库
        use crate::db::device_group;
        use sea_orm::EntityTrait;
        let active_model: device_group::ActiveModel = group.clone().into();
        device_group::Entity::insert(active_model)
            .exec(&*self.db)
            .await?;

        info!(
            group_id = %group.id,
            group_name = %group.name,
            parent_id = ?group.parent_id,
            "Device group created"
        );

        // 更新缓存
        if self.cache_enabled {
            let mut groups = self.groups.write().await;
            groups.insert(group.id.clone(), group.clone());
        }

        Ok(group)
    }

    /// 获取分组信息
    /// 
    /// # 参数
    /// * `group_id` - 分组ID
    /// 
    /// # 返回
    /// 分组信息，如果不存在则返回 None
    pub async fn get_group(&self, group_id: &str) -> Result<Option<DeviceGroup>> {
        // 先从缓存查询
        if self.cache_enabled {
            let groups = self.groups.read().await;
            if let Some(group) = groups.get(group_id) {
                debug!(group_id = %group_id, "Group found in cache");
                return Ok(Some(group.clone()));
            }
        }

        // 从数据库查询
        use crate::db::device_group;
        use sea_orm::EntityTrait;
        let model = device_group::Entity::find_by_id(group_id.to_string())
            .one(&*self.db)
            .await?;
        
        if let Some(model) = model {
            let group = DeviceGroup::from(model);
            
            // 更新缓存
            if self.cache_enabled {
                let mut groups = self.groups.write().await;
                groups.insert(group_id.to_string(), group.clone());
            }
            
            debug!(group_id = %group_id, "Group found in database");
            Ok(Some(group))
        } else {
            debug!(group_id = %group_id, "Group not found");
            Ok(None)
        }
    }

    /// 更新分组信息
    /// 
    /// # 参数
    /// * `group_id` - 分组ID
    /// * `group` - 新的分组信息
    /// 
    /// # 返回
    /// 更新后的分组信息
    pub async fn update_group(&self, group_id: &str, mut group: DeviceGroup) -> Result<DeviceGroup> {
        // 检查分组是否存在
        if !self.exists(group_id).await? {
            return Err(DeviceError::GroupNotFound(group_id.to_string()));
        }

        // 验证分组信息
        self.validate_group(&group)?;

        // 更新时间
        group.updated_at = chrono::Utc::now();

        // 更新到数据库
        use crate::db::device_group;
        use sea_orm::ActiveModelTrait;
        let active_model: device_group::ActiveModel = group.clone().into();
        active_model.update(&*self.db).await?;

        info!(group_id = %group_id, "Device group updated");

        // 更新缓存
        if self.cache_enabled {
            let mut groups = self.groups.write().await;
            groups.insert(group_id.to_string(), group.clone());
        }

        Ok(group)
    }

    /// 删除分组
    /// 
    /// # 参数
    /// * `group_id` - 分组ID
    /// 
    /// # 错误
    /// * `GroupNotFound` - 分组不存在
    /// * `ValidationError` - 分组下还有设备或子分组
    pub async fn delete_group(&self, group_id: &str) -> Result<()> {
        // 检查分组是否存在
        if !self.exists(group_id).await? {
            return Err(DeviceError::GroupNotFound(group_id.to_string()));
        }

        // 检查分组下是否有设备
        let devices = self.get_devices(group_id).await?;
        if !devices.is_empty() {
            return Err(DeviceError::validation(
                format!("Cannot delete group with {} devices", devices.len())
            ));
        }

        // 检查是否有子分组
        let children = self.get_children(group_id).await?;
        if !children.is_empty() {
            return Err(DeviceError::validation(
                format!("Cannot delete group with {} child groups", children.len())
            ));
        }

        // 从数据库删除
        use crate::db::device_group;
        use sea_orm::EntityTrait;
        device_group::Entity::delete_by_id(group_id.to_string())
            .exec(&*self.db)
            .await?;

        info!(group_id = %group_id, "Device group deleted");

        // 从缓存删除
        if self.cache_enabled {
            let mut groups = self.groups.write().await;
            groups.remove(group_id);
        }

        Ok(())
    }

    /// 列出所有分组
    pub async fn list_groups(&self) -> Result<Vec<DeviceGroup>> {
        use crate::db::device_group;
        use sea_orm::EntityTrait;
        
        // 从数据库查询所有分组
        let models = device_group::Entity::find()
            .all(&*self.db)
            .await?;
        
        let groups: Vec<DeviceGroup> = models.into_iter()
            .map(|m| DeviceGroup::from(m))
            .collect();
        
        debug!(count = groups.len(), "Groups listed from database");
        Ok(groups)
    }

    /// 获取子分组
    /// 
    /// # 参数
    /// * `parent_id` - 父分组ID
    /// 
    /// # 返回
    /// 子分组列表
    pub async fn get_children(&self, parent_id: &str) -> Result<Vec<DeviceGroup>> {
        use crate::db::device_group;
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
        
        // 从数据库查询子分组
        let models = device_group::Entity::find()
            .filter(device_group::Column::ParentId.eq(parent_id))
            .all(&*self.db)
            .await?;
        
        let children: Vec<DeviceGroup> = models.into_iter()
            .map(|m| DeviceGroup::from(m))
            .collect();
        
        Ok(children)
    }

    /// 添加设备到分组
    /// 
    /// # 参数
    /// * `group_id` - 分组ID
    /// * `device_id` - 设备ID
    pub async fn add_device(&self, group_id: &str, device_id: &str) -> Result<()> {
        // 检查分组是否存在
        if !self.exists(group_id).await? {
            return Err(DeviceError::GroupNotFound(group_id.to_string()));
        }

        // 检查设备是否存在
        let device = self.registry.get(device_id).await?;
        if device.is_none() {
            return Err(DeviceError::NotFound(device_id.to_string()));
        }

        // 更新设备的分组ID
        if let Some(mut device) = device {
            device.group_id = Some(group_id.to_string());
            self.registry.update(device_id, device).await?;
        }

        info!(
            group_id = %group_id,
            device_id = %device_id,
            "Device added to group"
        );

        Ok(())
    }

    /// 从分组移除设备
    /// 
    /// # 参数
    /// * `group_id` - 分组ID
    /// * `device_id` - 设备ID
    pub async fn remove_device(&self, group_id: &str, device_id: &str) -> Result<()> {
        // 检查设备是否存在
        let device = self.registry.get(device_id).await?;
        if device.is_none() {
            return Err(DeviceError::NotFound(device_id.to_string()));
        }

        // 更新设备的分组ID为 None
        if let Some(mut device) = device {
            if device.group_id.as_ref() != Some(&group_id.to_string()) {
                return Err(DeviceError::validation(
                    "Device is not in this group"
                ));
            }
            device.group_id = None;
            self.registry.update(device_id, device).await?;
        }

        info!(
            group_id = %group_id,
            device_id = %device_id,
            "Device removed from group"
        );

        Ok(())
    }

    /// 获取分组下的所有设备
    /// 
    /// # 参数
    /// * `group_id` - 分组ID
    /// 
    /// # 返回
    /// 设备列表
    pub async fn get_devices(&self, group_id: &str) -> Result<Vec<Device>> {
        let filter = DeviceFilter {
            group_id: Some(group_id.to_string()),
            ..Default::default()
        };
        self.registry.list(filter).await
    }

    /// 批量添加设备到分组
    /// 
    /// # 参数
    /// * `group_id` - 分组ID
    /// * `device_ids` - 设备ID列表
    /// 
    /// # 返回
    /// 成功添加的设备数量
    pub async fn add_devices_batch(&self, group_id: &str, device_ids: &[String]) -> Result<usize> {
        let mut success_count = 0;

        for device_id in device_ids {
            match self.add_device(group_id, device_id).await {
                Ok(_) => success_count += 1,
                Err(e) => {
                    warn!(
                        device_id = %device_id,
                        error = %e,
                        "Failed to add device to group"
                    );
                }
            }
        }

        info!(
            group_id = %group_id,
            total = device_ids.len(),
            success = success_count,
            "Batch add devices to group completed"
        );

        Ok(success_count)
    }

    /// 移动分组到新的父分组
    /// 
    /// # 参数
    /// * `group_id` - 要移动的分组ID
    /// * `new_parent_id` - 新的父分组ID（None 表示移动到根）
    pub async fn move_group(&self, group_id: &str, new_parent_id: Option<String>) -> Result<()> {
        // 获取分组
        let mut group = self.get_group(group_id).await?
            .ok_or_else(|| DeviceError::GroupNotFound(group_id.to_string()))?;

        // 如果有新父分组，检查是否存在
        if let Some(ref parent_id) = new_parent_id {
            if !self.exists(parent_id).await? {
                return Err(DeviceError::GroupNotFound(parent_id.clone()));
            }

            // 检查是否会形成循环
            if self.would_create_cycle(group_id, parent_id).await? {
                return Err(DeviceError::validation("Would create circular reference"));
            }

            // 更新路径
            if let Some(parent) = self.get_group(parent_id).await? {
                group.path = format!("{}/{}", parent.path, group.id);
            }
        } else {
            // 移动到根
            group.path = format!("/{}", group.id);
        }

        group.parent_id = new_parent_id;
        self.update_group(group_id, group).await?;

        info!(group_id = %group_id, "Group moved");
        Ok(())
    }

    /// 检查分组是否存在
    pub async fn exists(&self, group_id: &str) -> Result<bool> {
        // 先查缓存
        if self.cache_enabled {
            let groups = self.groups.read().await;
            if groups.contains_key(group_id) {
                return Ok(true);
            }
        }

        // 从数据库查询
        use crate::db::device_group;
        use sea_orm::{EntityTrait, PaginatorTrait};
        let count = device_group::Entity::find_by_id(group_id.to_string())
            .count(&*self.db)
            .await?;
        
        Ok(count > 0)
    }

    /// 统计分组数量
    pub async fn count(&self) -> Result<u64> {
        use crate::db::device_group;
        use sea_orm::{EntityTrait, PaginatorTrait};
        
        let count = device_group::Entity::find()
            .count(&*self.db)
            .await?;
        
        Ok(count)
    }

    // ========== 私有辅助方法 ==========

    /// 验证分组信息
    fn validate_group(&self, group: &DeviceGroup) -> Result<()> {
        if group.id.is_empty() {
            return Err(DeviceError::validation("Group ID cannot be empty"));
        }

        if group.name.is_empty() {
            return Err(DeviceError::validation("Group name cannot be empty"));
        }

        if group.name.len() > 255 {
            return Err(DeviceError::validation("Group name too long (max 255 characters)"));
        }

        Ok(())
    }

    /// 检查是否会形成循环引用
    async fn would_create_cycle(&self, group_id: &str, new_parent_id: &str) -> Result<bool> {
        let mut current_id = new_parent_id.to_string();

        // 向上遍历父分组链
        loop {
            if current_id == group_id {
                return Ok(true); // 会形成循环
            }

            match self.get_group(&current_id).await? {
                Some(group) => {
                    if let Some(parent_id) = group.parent_id {
                        current_id = parent_id;
                    } else {
                        break; // 到达根分组
                    }
                }
                None => break,
            }
        }

        Ok(false)
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

    async fn create_test_manager() -> DeviceGroupManager {
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
        
        let db = Arc::new(db);
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        DeviceGroupManager::new(db, registry)
    }

    #[tokio::test]
    async fn test_create_group() {
        let manager = create_test_manager().await;
        let group = DeviceGroup::new("一楼".to_string(), None);
        let result = manager.create_group(group).await;
        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created.name, "一楼");
        assert!(created.path.starts_with("/grp_"));
    }

    #[tokio::test]
    async fn test_create_child_group() {
        let manager = create_test_manager().await;

        // 创建父分组
        let parent = DeviceGroup::new("一楼".to_string(), None);
        let parent_id = parent.id.clone();
        manager.create_group(parent).await.unwrap();

        // 创建子分组
        let child = DeviceGroup::new("101房间".to_string(), Some(parent_id.clone()));
        let result = manager.create_group(child).await;

        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created.parent_id, Some(parent_id));
    }

    #[tokio::test]
    async fn test_add_device_to_group() {
        let manager = create_test_manager().await;
        let registry = manager.registry.clone();

        // 创建分组
        let group = DeviceGroup::new("一楼".to_string(), None);
        let group_id = group.id.clone();
        manager.create_group(group).await.unwrap();

        // 注册设备
        let device = create_test_device("测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();

        // 添加设备到分组
        let result = manager.add_device(&group_id, &device_id).await;
        assert!(result.is_ok());

        // 验证设备已添加到分组
        let devices = manager.get_devices(&group_id).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, device_id);
    }

    #[tokio::test]
    async fn test_remove_device_from_group() {
        let manager = create_test_manager().await;
        let registry = manager.registry.clone();

        // 创建分组并添加设备
        let group = DeviceGroup::new("一楼".to_string(), None);
        let group_id = group.id.clone();
        manager.create_group(group).await.unwrap();

        let device = create_test_device("测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();
        manager.add_device(&group_id, &device_id).await.unwrap();

        // 移除设备
        let result = manager.remove_device(&group_id, &device_id).await;
        assert!(result.is_ok());

        // 验证设备已移除
        let devices = manager.get_devices(&group_id).await.unwrap();
        assert_eq!(devices.len(), 0);
    }

    #[tokio::test]
    async fn test_get_children() {
        let manager = create_test_manager().await;

        // 创建父分组
        let parent = DeviceGroup::new("一楼".to_string(), None);
        let parent_id = parent.id.clone();
        manager.create_group(parent).await.unwrap();

        // 创建多个子分组
        for i in 1..=3 {
            let child = DeviceGroup::new(
                format!("{}房间", i * 100),
                Some(parent_id.clone()),
            );
            manager.create_group(child).await.unwrap();
        }

        // 获取子分组
        let children = manager.get_children(&parent_id).await.unwrap();
        assert_eq!(children.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_group_with_devices() {
        let manager = create_test_manager().await;
        let registry = manager.registry.clone();

        // 创建分组并添加设备
        let group = DeviceGroup::new("一楼".to_string(), None);
        let group_id = group.id.clone();
        manager.create_group(group).await.unwrap();

        let device = create_test_device("测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();
        manager.add_device(&group_id, &device_id).await.unwrap();

        // 尝试删除有设备的分组
        let result = manager.delete_group(&group_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_batch_add_devices() {
        let manager = create_test_manager().await;
        let registry = manager.registry.clone();

        // 创建分组
        let group = DeviceGroup::new("一楼".to_string(), None);
        let group_id = group.id.clone();
        manager.create_group(group).await.unwrap();

        // 注册多个设备
        let mut device_ids = Vec::new();
        for i in 0..5 {
            let device = create_test_device(&format!("设备{}", i));
            device_ids.push(device.id.clone());
            registry.register(device).await.unwrap();
        }

        // 批量添加
        let count = manager.add_devices_batch(&group_id, &device_ids).await.unwrap();
        assert_eq!(count, 5);

        // 验证
        let devices = manager.get_devices(&group_id).await.unwrap();
        assert_eq!(devices.len(), 5);
    }

    #[tokio::test]
    async fn test_move_group() {
        let manager = create_test_manager().await;

        // 创建分组结构
        let root = DeviceGroup::new("根分组".to_string(), None);
        let root_id = root.id.clone();
        manager.create_group(root).await.unwrap();

        let child = DeviceGroup::new("子分组".to_string(), None);
        let child_id = child.id.clone();
        manager.create_group(child).await.unwrap();

        // 移动子分组到根分组下
        let result = manager.move_group(&child_id, Some(root_id.clone())).await;
        assert!(result.is_ok());

        // 验证
        let moved = manager.get_group(&child_id).await.unwrap().unwrap();
        assert_eq!(moved.parent_id, Some(root_id));
    }
}
