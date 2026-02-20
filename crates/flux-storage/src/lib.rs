pub mod disk;
pub mod pool;
pub mod health;
pub mod metrics;
pub mod manager;

// 监控服务模块（可选编译）
#[cfg(feature = "monitor")]
pub mod monitor;

pub use disk::{DiskInfo, DiskType, DiskMonitor};
pub use pool::{StoragePool, PoolConfig};
pub use health::{HealthChecker, HealthStatus};
pub use metrics::StorageMetrics;
pub use manager::StorageManager;
