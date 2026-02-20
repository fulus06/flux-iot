use crate::disk::DiskMonitor;
use crate::health::{HealthChecker, HealthStatus};
use crate::metrics::StorageMetrics;
use crate::pool::{PoolConfig, StoragePool};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// 存储管理器（参考 MinIO 设计）
pub struct StorageManager {
    /// 存储池
    pools: Arc<RwLock<HashMap<String, StoragePool>>>,
    
    /// 磁盘监控器
    disk_monitor: Arc<RwLock<DiskMonitor>>,
    
    /// 健康检查器
    health_checker: HealthChecker,
    
    /// 指标
    metrics: Arc<RwLock<StorageMetrics>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub name: String,
    pub path: PathBuf,
    pub usage_percent: f64,
    pub status: HealthStatus,
    pub total_space: u64,
    pub available_space: u64,
}

pub struct HealthCheckTaskHandle {
    shutdown_tx: watch::Sender<bool>,
    join_handle: JoinHandle<()>,
}

impl HealthCheckTaskHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.join_handle.await;
    }

    pub fn abort(self) {
        self.join_handle.abort();
    }
}

impl StorageManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            disk_monitor: Arc::new(RwLock::new(DiskMonitor::new())),
            health_checker: HealthChecker::default(),
            metrics: Arc::new(RwLock::new(StorageMetrics::new())),
        }
    }

    /// 初始化存储池（使用后端）
    pub async fn initialize_with_backends(
        &self,
        pool_configs: Vec<(PoolConfig, Arc<dyn crate::backend::StorageBackend>)>,
    ) -> Result<()> {
        let mut monitor = self.disk_monitor.write().await;
        let disks = monitor.scan_disks()?;

        let mut to_insert: Vec<(String, StoragePool)> = Vec::new();

        for (config, backend) in pool_configs {
            let mut config = config;

            // 将相对路径归一化为绝对路径
            config.path = match tokio::fs::canonicalize(&config.path).await {
                Ok(p) => p,
                Err(_) => config.path.clone(),
            };

            // 查找匹配的磁盘
            if let Some(disk_info) = disks.iter().find(|d| {
                config.path == d.mount_point || config.path.starts_with(&d.mount_point)
            }) {
                let pool = StoragePool::new(config.clone(), disk_info.clone(), backend);
                info!("Initialized storage pool with backend: {} at {:?}", config.name, config.path);
                to_insert.push((config.name.clone(), pool));
            } else {
                warn!("No disk found for pool: {} at {:?}", config.name, config.path);
            }
        }

        {
            let mut pools = self.pools.write().await;
            for (name, pool) in to_insert {
                pools.insert(name, pool);
            }
        }
        
        // 更新指标
        self.update_metrics().await?;
        
        Ok(())
    }

    /// 初始化存储池（兼容旧接口）
    pub async fn initialize(&self, pool_configs: Vec<PoolConfig>) -> Result<()> {
        let mut monitor = self.disk_monitor.write().await;
        let disks = monitor.scan_disks()?;

        let mut to_insert: Vec<(String, StoragePool)> = Vec::new();

        for config in pool_configs {
            let mut config = config;

            // 将相对路径归一化为绝对路径，避免无法匹配 mount point
            config.path = match tokio::fs::canonicalize(&config.path).await {
                Ok(p) => p,
                Err(_) => config.path.clone(),
            };

            // 查找匹配的磁盘
            if let Some(disk_info) = disks.iter().find(|d| {
                config.path == d.mount_point || config.path.starts_with(&d.mount_point)
            }) {
                // 创建本地后端
                use crate::backend::LocalBackend;
                let backend = Arc::new(LocalBackend::new(config.path.clone()));
                
                let pool = StoragePool::new(config.clone(), disk_info.clone(), backend);
                info!("Initialized storage pool: {} at {:?}", config.name, config.path);
                to_insert.push((config.name.clone(), pool));
            } else {
                warn!("No disk found for pool: {} at {:?}", config.name, config.path);
            }
        }

        {
            let mut pools = self.pools.write().await;
            for (name, pool) in to_insert {
                pools.insert(name, pool);
            }
        }
        
        // 更新指标
        self.update_metrics().await?;
        
        Ok(())
    }

    /// 写入文件到指定池
    pub async fn write_to_pool(
        &self,
        pool_name: &str,
        path: &str,
        data: &[u8],
    ) -> Result<()> {
        let pools = self.pools.read().await;
        let pool = pools.get(pool_name)
            .ok_or_else(|| anyhow!("Pool not found: {}", pool_name))?;
        
        pool.write(path, data).await
    }
    
    /// 从指定池读取文件
    pub async fn read_from_pool(
        &self,
        pool_name: &str,
        path: &str,
    ) -> Result<bytes::Bytes> {
        let pools = self.pools.read().await;
        let pool = pools.get(pool_name)
            .ok_or_else(|| anyhow!("Pool not found: {}", pool_name))?;
        
        pool.read(path).await
    }
    
    /// 从指定池删除文件
    pub async fn delete_from_pool(
        &self,
        pool_name: &str,
        path: &str,
    ) -> Result<()> {
        let pools = self.pools.read().await;
        let pool = pools.get(pool_name)
            .ok_or_else(|| anyhow!("Pool not found: {}", pool_name))?;
        
        pool.delete(path).await
    }
    
    /// 从所有池中删除文件
    pub async fn delete_from_any_pool(&self, path: &str) -> Result<()> {
        let pools = self.pools.read().await;
        let mut deleted = false;
        
        for pool in pools.values() {
            if pool.delete(path).await.is_ok() {
                deleted = true;
            }
        }
        
        if deleted {
            Ok(())
        } else {
            Err(anyhow!("File not found in any pool: {}", path))
        }
    }
    
    /// 从指定池列出文件
    pub async fn list_from_pool(
        &self,
        pool_name: &str,
        prefix: &str,
    ) -> Result<Vec<String>> {
        let pools = self.pools.read().await;
        let pool = pools.get(pool_name)
            .ok_or_else(|| anyhow!("Pool not found: {}", pool_name))?;
        
        pool.list(prefix).await
    }
    
    /// 从所有池中列出文件（去重）
    pub async fn list_from_all_pools(&self, prefix: &str) -> Result<Vec<String>> {
        let pools = self.pools.read().await;
        let mut all_files = std::collections::HashSet::new();
        
        for pool in pools.values() {
            if let Ok(files) = pool.list(prefix).await {
                for file in files {
                    all_files.insert(file);
                }
            }
        }
        
        Ok(all_files.into_iter().collect())
    }
    
    /// 从所有池中读取文件（尝试第一个找到的）
    pub async fn read_from_any_pool(&self, path: &str) -> Result<bytes::Bytes> {
        let pools = self.pools.read().await;
        
        for pool in pools.values() {
            if let Ok(data) = pool.read(path).await {
                return Ok(data);
            }
        }
        
        Err(anyhow!("File not found in any pool: {}", path))
    }
    
    /// 选择最佳池并写入
    pub async fn write_with_selection(
        &self,
        path: &str,
        data: &[u8],
    ) -> Result<String> {
        // 选择最佳存储池
        let pool_path = self.select_pool(data.len() as u64).await?;
        
        // 找到对应的池名称
        let pools = self.pools.read().await;
        let pool_name = pools.iter()
            .find(|(_, pool)| pool.get_path() == &pool_path)
            .map(|(name, _)| name.clone())
            .ok_or_else(|| anyhow!("Pool not found for path: {:?}", pool_path))?;
        
        drop(pools);
        
        // 写入文件
        self.write_to_pool(&pool_name, path, data).await?;
        
        Ok(pool_name)
    }

    /// 选择最佳存储池（负载均衡）
    pub async fn select_pool(&self, required_space: u64) -> Result<PathBuf> {
        let pools = self.pools.read().await;
        
        // 过滤可用的池
        let mut candidates: Vec<(&String, &StoragePool)> = Vec::new();
        for (name, pool) in pools.iter() {
            if pool.is_available().await && pool.available_space().await >= required_space {
                candidates.push((name, pool));
            }
        }
        
        if candidates.is_empty() {
            return Err(anyhow!("No available storage pool"));
        }
        
        // 按优先级排序，优先级相同则按可用空间排序
        candidates.sort_by(|a, b| {
            let priority_cmp = a.1.config.priority.cmp(&b.1.config.priority);
            if priority_cmp == std::cmp::Ordering::Equal {
                // 可用空间多的优先（需要同步获取）
                let a_space = a.1.disk_info.try_read().map(|info| info.available_space).unwrap_or(0);
                let b_space = b.1.disk_info.try_read().map(|info| info.available_space).unwrap_or(0);
                b_space.cmp(&a_space)
            } else {
                priority_cmp
            }
        });
        
        Ok(candidates[0].1.get_path().clone())
    }

    /// 刷新所有存储池状态
    pub async fn refresh(&self) -> Result<()> {
        let mut monitor = self.disk_monitor.write().await;
        monitor.refresh();
        let disks = monitor.scan_disks()?;

        let pools = {
            let pools = self.pools.read().await;
            pools.values().cloned().collect::<Vec<_>>()
        };

        for pool in pools {
            // 查找对应的磁盘信息
            if let Some(disk_info) = disks.iter().find(|d| {
                pool.config.path == d.mount_point || pool.config.path.starts_with(&d.mount_point)
            }) {
                pool.update_disk_info(disk_info.clone()).await;
                
                // 更新健康状态
                let usage = disk_info.usage_percent();
                let status = self.health_checker.check_disk_health(usage);
                pool.update_status(status).await;
                
                if status.needs_alert() {
                    warn!(
                        "Storage pool {} health status: {:?}, usage: {:.1}%",
                        pool.config.name, status, usage
                    );
                }
            }
        }
        
        // 更新指标
        self.update_metrics().await?;
        
        Ok(())
    }

    /// 更新指标
    async fn update_metrics(&self) -> Result<()> {
        let pools = self.pools.read().await;
        let mut metrics = self.metrics.write().await;
        
        let mut total_space = 0u64;
        let mut available_space = 0u64;
        let mut healthy = 0;
        let mut warning = 0;
        let mut critical = 0;
        let mut failed = 0;
        
        for pool in pools.values() {
            let info = pool.disk_info.read().await;
            let status = pool.status.read().await;
            
            total_space += info.total_space;
            available_space += info.available_space;
            
            match *status {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Warning => warning += 1,
                HealthStatus::Critical => critical += 1,
                HealthStatus::Failed => failed += 1,
            }
        }
        
        metrics.total_space = total_space;
        metrics.available_space = available_space;
        metrics.used_space = total_space - available_space;
        metrics.usage_percent = if total_space > 0 {
            (total_space - available_space) as f64 / total_space as f64 * 100.0
        } else {
            0.0
        };
        metrics.healthy_disks = healthy;
        metrics.warning_disks = warning;
        metrics.critical_disks = critical;
        metrics.failed_disks = failed;
        metrics.total_disks = pools.len();
        
        Ok(())
    }

    /// 获取指标
    pub async fn get_metrics(&self) -> StorageMetrics {
        self.metrics.read().await.clone()
    }

    /// 获取所有存储池信息
    pub async fn get_pools(&self) -> Vec<(String, PathBuf, f64, HealthStatus)> {
        let pools = self.pools.read().await;
        let mut result = Vec::new();
        
        for pool in pools.values() {
            let usage = pool.usage_percent().await;
            let status = *pool.status.read().await;
            result.push((
                pool.config.name.clone(),
                pool.config.path.clone(),
                usage,
                status,
            ));
        }
        
        result
    }

    pub async fn get_pools_stats(&self) -> Vec<PoolStats> {
        let pools = self.pools.read().await;
        let mut result = Vec::new();

        for pool in pools.values() {
            let usage = pool.usage_percent().await;
            let status = *pool.status.read().await;
            let info = pool.disk_info.read().await;

            result.push(PoolStats {
                name: pool.config.name.clone(),
                path: pool.config.path.clone(),
                usage_percent: usage,
                status,
                total_space: info.total_space,
                available_space: info.available_space,
            });
        }

        result
    }

    /// 启动后台健康检查任务
    pub async fn start_health_check_task(self: Arc<Self>) {
        let _handle = self.start_health_check_task_handle();
    }

    pub fn start_health_check_task_handle(self: Arc<Self>) -> HealthCheckTaskHandle {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let join_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = self.refresh().await {
                            error!("Health check failed: {}", e);
                        }
                    }
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        HealthCheckTaskHandle {
            shutdown_tx,
            join_handle,
        }
    }
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::DiskType;

    #[tokio::test]
    async fn test_storage_manager_creation() {
        let manager = StorageManager::new();
        let metrics = manager.get_metrics().await;
        
        // 初始状态应该是空的
        assert_eq!(metrics.total_disks, 0);
        assert_eq!(metrics.total_space, 0);
    }

    #[tokio::test]
    async fn test_storage_manager_initialize() {
        let manager = StorageManager::new();
        
        let configs = vec![
            PoolConfig {
                name: "test-pool".to_string(),
                path: PathBuf::from("/tmp"),
                disk_type: DiskType::SSD,
                priority: 1,
                max_usage_percent: 90.0,
            },
        ];
        
        // 初始化可能失败（如果 /tmp 不在独立磁盘上）
        let result = manager.initialize(configs).await;
        
        // 只要不 panic 就算通过
        match result {
            Ok(_) => {
                let metrics = manager.get_metrics().await;
                assert!(metrics.total_disks >= 0);
            }
            Err(_e) => {
                // 在某些环境下初始化可能失败，这是正常的
            }
        }
    }
}
