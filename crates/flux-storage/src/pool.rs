use crate::backend::StorageBackend;
use crate::disk::{DiskInfo, DiskType};
use crate::health::HealthStatus;
use anyhow::Result;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 存储池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub name: String,
    pub path: PathBuf,
    pub disk_type: DiskType,
    pub priority: u8,
    pub max_usage_percent: f64,
}

/// 存储池
#[derive(Clone)]
pub struct StoragePool {
    pub id: String,
    pub config: PoolConfig,
    pub disk_info: Arc<RwLock<DiskInfo>>,
    pub status: Arc<RwLock<HealthStatus>>,
    pub backend: Arc<dyn StorageBackend>,
}

impl StoragePool {
    pub fn new(config: PoolConfig, disk_info: DiskInfo, backend: Arc<dyn StorageBackend>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            disk_info: Arc::new(RwLock::new(disk_info)),
            status: Arc::new(RwLock::new(HealthStatus::Healthy)),
            backend,
        }
    }

    /// 获取可用空间
    pub async fn available_space(&self) -> u64 {
        self.disk_info.read().await.available_space
    }

    /// 获取使用率
    pub async fn usage_percent(&self) -> f64 {
        self.disk_info.read().await.usage_percent()
    }

    /// 是否可用
    pub async fn is_available(&self) -> bool {
        let info = self.disk_info.read().await;
        let status = self.status.read().await;

        info.is_available()
            && *status != HealthStatus::Failed
            && info.usage_percent() < self.config.max_usage_percent
    }

    /// 更新磁盘信息
    pub async fn update_disk_info(&self, new_info: DiskInfo) {
        let mut info = self.disk_info.write().await;
        *info = new_info;
    }

    /// 更新健康状态
    pub async fn update_status(&self, new_status: HealthStatus) {
        let mut status = self.status.write().await;
        *status = new_status;
    }

    /// 获取存储路径
    pub fn get_path(&self) -> &PathBuf {
        &self.config.path
    }
    
    /// 写入文件（使用 Backend）
    pub async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        self.backend.write(path, data).await
    }
    
    /// 读取文件（使用 Backend）
    pub async fn read(&self, path: &str) -> Result<Bytes> {
        self.backend.read(path).await
    }
    
    /// 删除文件（使用 Backend）
    pub async fn delete(&self, path: &str) -> Result<()> {
        self.backend.delete(path).await
    }
    
    /// 列出文件（使用 Backend）
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        self.backend.list(prefix).await
    }
    
    /// 检查文件是否存在（使用 Backend）
    pub async fn exists(&self, path: &str) -> Result<bool> {
        self.backend.exists(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::LocalBackend;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_pool() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalBackend::new(temp_dir.path().to_path_buf()));
        
        let config = PoolConfig {
            name: "test-pool".to_string(),
            path: temp_dir.path().to_path_buf(),
            disk_type: DiskType::SSD,
            priority: 1,
            max_usage_percent: 90.0,
        };

        let disk_info = DiskInfo {
            name: "test-disk".to_string(),
            mount_point: temp_dir.path().to_path_buf(),
            total_space: 1000_000_000_000, // 1 TB
            available_space: 500_000_000_000, // 500 GB
            disk_type: DiskType::SSD,
            file_system: "ext4".to_string(),
        };

        let pool = StoragePool::new(config, disk_info, backend);

        assert_eq!(pool.available_space().await, 500_000_000_000);
        assert!(pool.usage_percent().await < 60.0);
        assert!(pool.is_available().await);
    }
    
    #[tokio::test]
    async fn test_storage_pool_backend_operations() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalBackend::new(temp_dir.path().to_path_buf()));
        
        let config = PoolConfig {
            name: "test-pool".to_string(),
            path: temp_dir.path().to_path_buf(),
            disk_type: DiskType::SSD,
            priority: 1,
            max_usage_percent: 90.0,
        };

        let disk_info = DiskInfo {
            name: "test-disk".to_string(),
            mount_point: temp_dir.path().to_path_buf(),
            total_space: 1000_000_000_000,
            available_space: 500_000_000_000,
            disk_type: DiskType::SSD,
            file_system: "ext4".to_string(),
        };

        let pool = StoragePool::new(config, disk_info, backend);
        
        // 测试写入
        pool.write("test.txt", b"Hello, World!").await.unwrap();
        
        // 测试读取
        let data = pool.read("test.txt").await.unwrap();
        assert_eq!(&data[..], b"Hello, World!");
        
        // 测试存在性检查
        assert!(pool.exists("test.txt").await.unwrap());
        
        // 测试删除
        pool.delete("test.txt").await.unwrap();
        assert!(!pool.exists("test.txt").await.unwrap());
    }
}
