// 存储引擎：支持单节点和分布式双模式
use crate::Result;
use crate::VideoError;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// 存储引擎（双模式）
pub enum StorageEngine {
    /// 单节点模式（极致轻量）
    Standalone(super::StandaloneStorage),

    /// 多后端模式（NAS/NVR/对象存储等）
    Backend(Arc<dyn super::backend::StorageBackend>),
}

impl StorageEngine {
    /// 创建单节点模式
    pub fn standalone(base_path: std::path::PathBuf) -> Result<Self> {
        let storage = super::StandaloneStorage::new(base_path)?;
        Ok(Self::Standalone(storage))
    }

    /// 创建多后端模式
    pub fn backend(backend: Arc<dyn super::backend::StorageBackend>) -> Self {
        Self::Backend(backend)
    }

    /// 保存视频分片
    pub async fn save_segment(
        &mut self,
        stream_id: &str,
        timestamp: DateTime<Utc>,
        data: Bytes,
    ) -> Result<String> {
        match self {
            Self::Standalone(storage) => storage.put_object(stream_id, timestamp, data).await,
            Self::Backend(backend) => backend.save_segment(stream_id, timestamp, data).await,
        }
    }

    /// 读取视频分片
    pub async fn get_segment(&self, url: &str) -> Result<Bytes> {
        match self {
            Self::Standalone(_) => {
                // StandaloneStorage 的对象路径就是本地文件路径，因此这里直接读取。
                let data = tokio::fs::read(url).await.map_err(|e| {
                    VideoError::StorageError(format!("Failed to read segment {}: {}", url, e))
                })?;
                Ok(Bytes::from(data))
            }
            Self::Backend(backend) => backend.get_segment(url).await,
        }
    }

    /// 列出某个流在时间范围内的分片
    pub async fn list_segments(
        &self,
        stream_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<super::standalone::ObjectMetadata>> {
        match self {
            Self::Standalone(storage) => storage.list_objects(stream_id, start, end).await,
            Self::Backend(backend) => backend.list_segments(stream_id, start, end).await,
        }
    }

    /// 清理过期数据
    pub async fn cleanup_expired(&self, before: DateTime<Utc>) -> Result<usize> {
        match self {
            Self::Standalone(storage) => storage.cleanup_expired(before).await,
            Self::Backend(backend) => backend.cleanup_expired(before).await,
        }
    }
}
