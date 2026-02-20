pub mod backend;
pub mod disk;
pub mod pool;
pub mod health;
pub mod metrics;
pub mod manager;
pub mod segment;

// 监控服务模块（可选编译）
#[cfg(feature = "monitor")]
pub mod monitor;

pub use backend::{StorageBackend, LocalBackend, FileMetadata, BackendStats};
pub use disk::{DiskInfo, DiskType, DiskMonitor};
pub use pool::{StoragePool, PoolConfig};
pub use health::{HealthChecker, HealthStatus};
pub use metrics::StorageMetrics;
pub use manager::StorageManager;
pub use segment::{SegmentStorage, SegmentStorageImpl, LocalSegmentStorage};
