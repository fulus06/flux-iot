// 存储后端抽象

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use crate::Result;

pub mod local_fs;
pub mod router;

/// 存储后端抽象 trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 保存视频分片
    async fn save_segment(
        &self,
        stream_id: &str,
        timestamp: DateTime<Utc>,
        data: Bytes,
    ) -> Result<String>;
    
    /// 读取视频分片
    async fn get_segment(&self, url: &str) -> Result<Bytes>;

    /// 列出某个流在时间范围内的分片
    async fn list_segments(
        &self,
        stream_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<super::standalone::ObjectMetadata>>;

    /// 清理过期数据
    async fn cleanup_expired(&self, before: DateTime<Utc>) -> Result<usize>;
}

pub use local_fs::LocalFsBackend;
pub use router::StorageRouter;
pub use super::standalone::ObjectMetadata;
