pub mod backend;
pub mod disk;
pub mod pool;
pub mod health;
pub mod metrics;
pub mod manager;
pub mod segment;

// PostgreSQL 元数据后端（可选编译）
#[cfg(feature = "postgres")]
pub mod metadata_pg;

// 监控服务模块（可选编译）
#[cfg(feature = "monitor")]
pub mod monitor;

pub use backend::{StorageBackend, LocalBackend, FileMetadata, BackendStats};

#[cfg(feature = "s3")]
pub use backend::{S3Backend, S3Config};
pub use disk::{DiskInfo, DiskType, DiskMonitor};
pub use pool::{StoragePool, PoolConfig};
pub use health::{HealthChecker, HealthStatus};
pub use metrics::StorageMetrics;
pub use manager::StorageManager;
pub use segment::{LocalSegmentStorage, SegmentMetadata, SegmentStorage, SegmentStorageImpl};

#[cfg(feature = "postgres")]
pub use metadata_pg::PostgresMetadataBackend;
