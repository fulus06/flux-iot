use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("Cleanup failed: {0}")]
    CleanupFailed(String),
    
    #[error("Resource not found: {0}")]
    NotFound(String),
}

/// 资源接口
#[async_trait]
pub trait Resource: Send + Sync {
    /// 清理资源
    async fn cleanup(&self) -> Result<(), ResourceError>;
    
    /// 资源名称
    fn name(&self) -> &str;
    
    /// 清理优先级（数字越小优先级越高）
    fn priority(&self) -> u32 {
        100
    }
}

/// 资源管理器
pub struct ResourceManager {
    resources: Vec<Arc<dyn Resource>>,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
        }
    }

    /// 注册资源
    pub fn register(&mut self, resource: Arc<dyn Resource>) {
        info!("Registering resource: {}", resource.name());
        self.resources.push(resource);
    }

    /// 清理所有资源
    pub async fn cleanup_all(&mut self) {
        // 按优先级排序
        self.resources.sort_by_key(|r| r.priority());

        info!("Cleaning up {} resources", self.resources.len());

        for resource in &self.resources {
            info!("Cleaning up resource: {}", resource.name());
            
            match resource.cleanup().await {
                Ok(_) => {
                    info!("Successfully cleaned up: {}", resource.name());
                }
                Err(e) => {
                    error!("Failed to cleanup {}: {}", resource.name(), e);
                }
            }
        }

        info!("Resource cleanup complete");
    }

    /// 获取资源数量
    pub fn count(&self) -> usize {
        self.resources.len()
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 简单的文件资源示例
pub struct FileResource {
    name: String,
    path: String,
}

impl FileResource {
    pub fn new(name: String, path: String) -> Self {
        Self { name, path }
    }
}

#[async_trait]
impl Resource for FileResource {
    async fn cleanup(&self) -> Result<(), ResourceError> {
        info!("Syncing file: {}", self.path);
        // 实际实现中应该调用 file.sync_all()
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> u32 {
        50 // 文件资源优先级较高
    }
}

/// 数据库连接资源示例
pub struct DatabaseResource {
    name: String,
}

impl DatabaseResource {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Resource for DatabaseResource {
    async fn cleanup(&self) -> Result<(), ResourceError> {
        info!("Closing database connection");
        // 实际实现中应该调用 pool.close()
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> u32 {
        10 // 数据库资源优先级最高
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestResource {
        name: String,
        should_fail: bool,
    }

    #[async_trait]
    impl Resource for TestResource {
        async fn cleanup(&self) -> Result<(), ResourceError> {
            if self.should_fail {
                Err(ResourceError::CleanupFailed("Test failure".to_string()))
            } else {
                Ok(())
            }
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_resource_manager() {
        let mut manager = ResourceManager::new();

        let resource1 = Arc::new(TestResource {
            name: "resource1".to_string(),
            should_fail: false,
        });

        let resource2 = Arc::new(TestResource {
            name: "resource2".to_string(),
            should_fail: false,
        });

        manager.register(resource1);
        manager.register(resource2);

        assert_eq!(manager.count(), 2);

        manager.cleanup_all().await;
    }

    #[tokio::test]
    async fn test_cleanup_with_failure() {
        let mut manager = ResourceManager::new();

        let resource = Arc::new(TestResource {
            name: "failing_resource".to_string(),
            should_fail: true,
        });

        manager.register(resource);
        
        // 即使失败也应该继续
        manager.cleanup_all().await;
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let mut manager = ResourceManager::new();

        let db = Arc::new(DatabaseResource::new("database".to_string()));
        let file = Arc::new(FileResource::new("file".to_string(), "/tmp/test".to_string()));

        manager.register(file);
        manager.register(db);

        // 清理时应该按优先级排序（数据库先于文件）
        manager.cleanup_all().await;
    }
}
