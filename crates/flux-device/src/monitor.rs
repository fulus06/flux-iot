use crate::{Device, DeviceError, DeviceMetrics, DeviceRegistry, DeviceStatus, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// 设备监控器
/// 
/// 负责设备心跳检测、状态追踪和健康检查
pub struct DeviceMonitor {
    /// 设备注册表
    registry: Arc<DeviceRegistry>,
    
    /// 心跳间隔（设备应该发送心跳的间隔）
    heartbeat_interval: Duration,
    
    /// 超时时间（超过此时间未收到心跳则认为离线）
    timeout: Duration,
    
    /// 设备最后心跳时间
    last_heartbeat: Arc<RwLock<HashMap<String, chrono::DateTime<chrono::Utc>>>>,
    
    /// 是否正在运行
    running: Arc<RwLock<bool>>,
}

impl DeviceMonitor {
    /// 创建新的设备监控器
    /// 
    /// # 参数
    /// * `registry` - 设备注册表
    /// * `heartbeat_interval` - 心跳间隔（秒）
    /// * `timeout` - 超时时间（秒）
    pub fn new(
        registry: Arc<DeviceRegistry>,
        heartbeat_interval: u64,
        timeout: u64,
    ) -> Self {
        Self {
            registry,
            heartbeat_interval: Duration::from_secs(heartbeat_interval),
            timeout: Duration::from_secs(timeout),
            last_heartbeat: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动监控器
    /// 
    /// 启动后台任务，定期检查设备心跳超时
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            warn!("Device monitor is already running");
            return;
        }
        *running = true;
        drop(running);

        info!(
            heartbeat_interval = ?self.heartbeat_interval,
            timeout = ?self.timeout,
            "Device monitor started"
        );

        // 启动心跳检查任务
        let registry = self.registry.clone();
        let last_heartbeat = self.last_heartbeat.clone();
        let timeout = self.timeout;
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(10)); // 每10秒检查一次

            loop {
                check_interval.tick().await;

                // 检查是否应该停止
                let is_running = *running.read().await;
                if !is_running {
                    info!("Device monitor stopped");
                    break;
                }

                // 检查所有设备的心跳超时
                Self::check_heartbeat_timeout(
                    registry.clone(),
                    last_heartbeat.clone(),
                    timeout,
                )
                .await;
            }
        });
    }

    /// 停止监控器
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Device monitor stopping...");
    }

    /// 记录设备心跳
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// 
    /// # 错误
    /// * `NotFound` - 设备不存在
    pub async fn heartbeat(&self, device_id: &str) -> Result<()> {
        // 检查设备是否存在
        let device = self.registry.get(device_id).await?;
        if device.is_none() {
            return Err(DeviceError::NotFound(device_id.to_string()));
        }

        let now = chrono::Utc::now();

        // 更新最后心跳时间
        let mut heartbeats = self.last_heartbeat.write().await;
        heartbeats.insert(device_id.to_string(), now);

        // 更新设备状态为在线
        if let Some(mut device) = device {
            if device.status != DeviceStatus::Online {
                device.set_status(DeviceStatus::Online);
                device.update_last_seen();
                self.registry.update(device_id, device).await?;
                info!(device_id = %device_id, "Device came online");
            } else {
                device.update_last_seen();
                self.registry.update(device_id, device).await?;
            }
        }

        debug!(device_id = %device_id, "Heartbeat received");
        Ok(())
    }

    /// 获取设备状态
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// 
    /// # 返回
    /// 设备当前状态
    pub async fn get_status(&self, device_id: &str) -> Result<DeviceStatus> {
        let device = self.registry.get(device_id).await?;
        match device {
            Some(d) => Ok(d.status),
            None => Err(DeviceError::NotFound(device_id.to_string())),
        }
    }

    /// 设置设备状态
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// * `status` - 新状态
    pub async fn set_status(&self, device_id: &str, status: DeviceStatus) -> Result<()> {
        let device = self.registry.get(device_id).await?;
        match device {
            Some(mut d) => {
                let old_status = d.status.clone();
                d.set_status(status.clone());
                self.registry.update(device_id, d).await?;
                
                info!(
                    device_id = %device_id,
                    old_status = ?old_status,
                    new_status = ?status,
                    "Device status changed"
                );
                Ok(())
            }
            None => Err(DeviceError::NotFound(device_id.to_string())),
        }
    }

    /// 获取设备指标
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// 
    /// # 返回
    /// 设备指标列表
    pub async fn get_metrics(&self, device_id: &str) -> Result<Vec<DeviceMetrics>> {
        use crate::db::device_metrics;
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, QueryOrder, QuerySelect};
        
        // 检查设备是否存在
        let device = self.registry.get(device_id).await?;
        if device.is_none() {
            return Err(DeviceError::NotFound(device_id.to_string()));
        }

        // 从数据库查询指标（最近100条）
        let models = device_metrics::Entity::find()
            .filter(device_metrics::Column::DeviceId.eq(device_id))
            .order_by_desc(device_metrics::Column::Timestamp)
            .limit(100)
            .all(&*self.registry.db)
            .await?;
        
        // 转换为 DeviceMetrics
        let metrics: Vec<DeviceMetrics> = models.into_iter()
            .map(|m| DeviceMetrics::from(m))
            .collect();
        
        Ok(metrics)
    }

    /// 记录设备指标
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// * `metric_name` - 指标名称
    /// * `metric_value` - 指标值
    /// * `unit` - 单位（可选）
    pub async fn record_metric(
        &self,
        device_id: &str,
        metric_name: String,
        metric_value: f64,
        unit: Option<String>,
    ) -> Result<()> {
        use crate::db::device_metrics;
        use sea_orm::{EntityTrait, Set};
        
        // 检查设备是否存在
        let device = self.registry.get(device_id).await?;
        if device.is_none() {
            return Err(DeviceError::NotFound(device_id.to_string()));
        }

        // 创建指标记录
        let metric = DeviceMetrics {
            id: 0, // 自动生成
            device_id: device_id.to_string(),
            metric_name: metric_name.clone(),
            metric_value,
            unit: unit.clone(),
            timestamp: chrono::Utc::now(),
        };
        
        // 保存到数据库
        let active_model: device_metrics::ActiveModel = metric.into();
        device_metrics::Entity::insert(active_model)
            .exec(&*self.registry.db)
            .await?;
        
        debug!(
            device_id = %device_id,
            metric_name = %metric_name,
            metric_value = %metric_value,
            "Metric recorded to database"
        );

        Ok(())
    }

    /// 检查设备是否在线
    /// 
    /// # 参数
    /// * `device_id` - 设备ID
    /// 
    /// # 返回
    /// 如果设备在线返回 true，否则返回 false
    pub async fn is_online(&self, device_id: &str) -> Result<bool> {
        let status = self.get_status(device_id).await?;
        Ok(status == DeviceStatus::Online)
    }

    /// 获取在线设备数量
    pub async fn online_count(&self) -> Result<u64> {
        // TODO: 优化查询
        let filter = crate::DeviceFilter {
            status: Some(DeviceStatus::Online),
            ..Default::default()
        };
        self.registry.count(filter).await
    }

    /// 获取离线设备数量
    pub async fn offline_count(&self) -> Result<u64> {
        let filter = crate::DeviceFilter {
            status: Some(DeviceStatus::Offline),
            ..Default::default()
        };
        self.registry.count(filter).await
    }

    // ========== 私有辅助方法 ==========

    /// 检查心跳超时
    async fn check_heartbeat_timeout(
        registry: Arc<DeviceRegistry>,
        last_heartbeat: Arc<RwLock<HashMap<String, chrono::DateTime<chrono::Utc>>>>,
        timeout: Duration,
    ) {
        let now = chrono::Utc::now();
        let heartbeats = last_heartbeat.read().await;

        for (device_id, last_time) in heartbeats.iter() {
            let elapsed = now.signed_duration_since(*last_time);
            
            if elapsed.num_seconds() > timeout.as_secs() as i64 {
                // 心跳超时，标记设备为离线
                if let Ok(Some(mut device)) = registry.get(device_id).await {
                    if device.status == DeviceStatus::Online {
                        device.set_status(DeviceStatus::Offline);
                        if let Err(e) = registry.update(device_id, device).await {
                            warn!(
                                device_id = %device_id,
                                error = %e,
                                "Failed to update device status to offline"
                            );
                        } else {
                            warn!(device_id = %device_id, "Device went offline (heartbeat timeout)");
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceType, Protocol};
    use sea_orm::{Database, ConnectionTrait, Statement};

    fn create_test_device(_id: &str, name: &str) -> Device {
        Device::new(
            name.to_string(),
            DeviceType::Sensor,
            Protocol::MQTT,
        )
    }

    async fn create_test_monitor() -> DeviceMonitor {
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
        
        let db = Arc::new(db);
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        DeviceMonitor::new(registry, 30, 60)
    }

    #[tokio::test]
    async fn test_heartbeat() {
        let monitor = create_test_monitor().await;
        let registry = monitor.registry.clone();
        let monitor = DeviceMonitor::new(registry.clone(), 30, 60);

        // 注册设备
        let device = create_test_device("test_001", "测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();

        // 发送心跳
        let result = monitor.heartbeat(&device_id).await;
        assert!(result.is_ok());

        // 验证设备状态变为在线
        let status = monitor.get_status(&device_id).await.unwrap();
        assert_eq!(status, DeviceStatus::Online);
    }

    #[tokio::test]
    async fn test_heartbeat_nonexistent_device() {
        let monitor = create_test_monitor().await;

        // 尝试为不存在的设备发送心跳
        let result = monitor.heartbeat("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DeviceError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_set_status() {
        let monitor = create_test_monitor().await;
        let registry = monitor.registry.clone();

        // 注册设备
        let device = create_test_device("test_001", "测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();

        // 设置状态为维护中
        monitor.set_status(&device_id, DeviceStatus::Maintenance).await.unwrap();

        // 验证状态
        let status = monitor.get_status(&device_id).await.unwrap();
        assert_eq!(status, DeviceStatus::Maintenance);
    }

    #[tokio::test]
    async fn test_is_online() {
        let monitor = create_test_monitor().await;
        let registry = monitor.registry.clone();

        // 注册设备
        let device = create_test_device("test_001", "测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();

        // 初始状态应该是离线
        let is_online = monitor.is_online(&device_id).await.unwrap();
        assert!(!is_online);

        // 发送心跳后应该在线
        monitor.heartbeat(&device_id).await.unwrap();
        let is_online = monitor.is_online(&device_id).await.unwrap();
        assert!(is_online);
    }

    #[tokio::test]
    async fn test_record_metric() {
        let monitor = create_test_monitor().await;
        let registry = monitor.registry.clone();

        // 注册设备
        let device = create_test_device("test_001", "测试设备");
        let device_id = device.id.clone();
        registry.register(device).await.unwrap();

        // 记录指标
        let result = monitor.record_metric(
            &device_id,
            "temperature".to_string(),
            25.5,
            Some("°C".to_string()),
        ).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_online_count() {
        let monitor = create_test_monitor().await;
        let registry = monitor.registry.clone();

        // 注册多个设备
        for i in 0..3 {
            let device = create_test_device(&format!("test_{:03}", i), &format!("设备{}", i));
            let device_id = device.id.clone();
            registry.register(device).await.unwrap();
            
            // 前两个设备发送心跳（在线）
            if i < 2 {
                monitor.heartbeat(&device_id).await.unwrap();
            }
        }

        // 验证在线设备数量
        let count = monitor.online_count().await.unwrap();
        assert_eq!(count, 2);

        // 验证离线设备数量
        let count = monitor.offline_count().await.unwrap();
        assert_eq!(count, 0); // Inactive 状态不算 Offline
    }

    #[tokio::test]
    async fn test_monitor_start_stop() {
        let monitor = create_test_monitor().await;

        // 启动监控器
        monitor.start().await;
        
        // 等待一小段时间
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 停止监控器
        monitor.stop().await;
        
        // 等待停止完成
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
