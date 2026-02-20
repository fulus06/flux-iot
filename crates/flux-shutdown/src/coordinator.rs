use crate::connection::ConnectionTracker;
use crate::resource::ResourceManager;
use crate::signal::{ShutdownSignal, SignalHandler};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// 关闭阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownPhase {
    Running,
    Preparing,
    Draining,
    Cleaning,
    Complete,
}

/// 关闭协调器
pub struct ShutdownCoordinator {
    signal_handler: SignalHandler,
    connection_tracker: Option<ConnectionTracker>,
    resource_manager: Option<ResourceManager>,
    shutdown_timeout: Duration,
    drain_timeout: Duration,
}

impl ShutdownCoordinator {
    pub fn builder() -> ShutdownCoordinatorBuilder {
        ShutdownCoordinatorBuilder::new()
    }

    /// 运行关闭流程
    pub async fn run(mut self) -> ShutdownPhase {
        info!("Shutdown coordinator started, waiting for signal...");

        // Phase 1: 等待关闭信号
        let signal = self.signal_handler.wait_for_signal().await;
        info!("Received shutdown signal: {:?}", signal);

        let start = std::time::Instant::now();

        // Phase 2: 排空连接
        if let Some(tracker) = &self.connection_tracker {
            info!("Phase 2: Draining connections...");
            
            match timeout(self.drain_timeout, tracker.drain()).await {
                Ok(_) => {
                    info!("Connections drained successfully");
                }
                Err(_) => {
                    warn!("Connection drain timed out after {:?}", self.drain_timeout);
                }
            }
        }

        // Phase 3: 清理资源
        if let Some(manager) = &mut self.resource_manager {
            info!("Phase 3: Cleaning up resources...");
            manager.cleanup_all().await;
        }

        let elapsed = start.elapsed();
        info!("Graceful shutdown complete in {:?}", elapsed);

        ShutdownPhase::Complete
    }

    /// 获取信号处理器的引用
    pub fn signal_handler(&self) -> &SignalHandler {
        &self.signal_handler
    }
}

/// 关闭协调器构建器
pub struct ShutdownCoordinatorBuilder {
    signal_handler: Option<SignalHandler>,
    connection_tracker: Option<ConnectionTracker>,
    resource_manager: Option<ResourceManager>,
    shutdown_timeout: Duration,
    drain_timeout: Duration,
}

impl ShutdownCoordinatorBuilder {
    pub fn new() -> Self {
        Self {
            signal_handler: None,
            connection_tracker: None,
            resource_manager: None,
            shutdown_timeout: Duration::from_secs(60),
            drain_timeout: Duration::from_secs(30),
        }
    }

    pub fn with_signal_handler(mut self, handler: SignalHandler) -> Self {
        self.signal_handler = Some(handler);
        self
    }

    pub fn with_connection_tracker(mut self, tracker: ConnectionTracker) -> Self {
        self.connection_tracker = Some(tracker);
        self
    }

    pub fn with_resource_manager(mut self, manager: ResourceManager) -> Self {
        self.resource_manager = Some(manager);
        self
    }

    pub fn with_shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    pub fn with_drain_timeout(mut self, timeout: Duration) -> Self {
        self.drain_timeout = timeout;
        self
    }

    pub fn build(self) -> ShutdownCoordinator {
        let signal_handler = self.signal_handler.unwrap_or_else(|| {
            SignalHandler::new().0
        });

        ShutdownCoordinator {
            signal_handler,
            connection_tracker: self.connection_tracker,
            resource_manager: self.resource_manager,
            shutdown_timeout: self.shutdown_timeout,
            drain_timeout: self.drain_timeout,
        }
    }
}

impl Default for ShutdownCoordinatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{Resource, ResourceError};
    use async_trait::async_trait;
    use std::sync::Arc;

    struct TestResource {
        name: String,
    }

    #[async_trait]
    impl Resource for TestResource {
        async fn cleanup(&self) -> Result<(), ResourceError> {
            Ok(())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_coordinator_builder() {
        let (handler, _rx) = SignalHandler::new();
        let tracker = ConnectionTracker::new(Duration::from_secs(5));
        let mut manager = ResourceManager::new();
        
        manager.register(Arc::new(TestResource {
            name: "test".to_string(),
        }));

        let coordinator = ShutdownCoordinator::builder()
            .with_signal_handler(handler)
            .with_connection_tracker(tracker)
            .with_resource_manager(manager)
            .with_shutdown_timeout(Duration::from_secs(30))
            .with_drain_timeout(Duration::from_secs(10))
            .build();

        // 验证构建成功
        assert!(coordinator.connection_tracker.is_some());
        assert!(coordinator.resource_manager.is_some());
    }

    #[tokio::test]
    async fn test_coordinator_shutdown_phases() {
        // 测试关闭阶段枚举
        assert_eq!(ShutdownPhase::Running, ShutdownPhase::Running);
        assert_eq!(ShutdownPhase::Complete, ShutdownPhase::Complete);
        assert_ne!(ShutdownPhase::Running, ShutdownPhase::Complete);
    }
}
