// 本地文件系统后端（可作为 NAS/NVR 后端的基础实现）

use super::StorageBackend;
use crate::{Result, VideoError};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 基于 StandaloneStorage 的本地文件系统后端
///
/// 注意：StandaloneStorage 的写入接口需要 `&mut self`，因此这里使用 Mutex 做内部可变性封装。
pub struct LocalFsBackend {
    storage: Arc<Mutex<crate::storage::StandaloneStorage>>,
}

impl LocalFsBackend {
    pub fn new(storage: crate::storage::StandaloneStorage) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
        }
    }

    pub fn from_shared(storage: Arc<Mutex<crate::storage::StandaloneStorage>>) -> Self {
        Self { storage }
    }

    pub fn inner(&self) -> &Arc<Mutex<crate::storage::StandaloneStorage>> {
        &self.storage
    }
}

#[async_trait]
impl StorageBackend for LocalFsBackend {
    async fn save_segment(
        &self,
        stream_id: &str,
        timestamp: DateTime<Utc>,
        data: Bytes,
    ) -> Result<String> {
        let mut storage = self.storage.lock().await;
        storage.put_object(stream_id, timestamp, data).await
    }

    async fn get_segment(&self, url: &str) -> Result<Bytes> {
        // 当前实现约定 url 为本地文件绝对/相对路径。
        let data = tokio::fs::read(url).await.map_err(|e| {
            VideoError::StorageError(format!("Failed to read segment {}: {}", url, e))
        })?;
        Ok(Bytes::from(data))
    }

    async fn list_segments(
        &self,
        stream_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<super::ObjectMetadata>> {
        let storage = self.storage.lock().await;
        storage.list_objects(stream_id, start, end).await
    }

    async fn cleanup_expired(&self, before: DateTime<Utc>) -> Result<usize> {
        let storage = self.storage.lock().await;
        storage.cleanup_expired(before).await
    }
}
