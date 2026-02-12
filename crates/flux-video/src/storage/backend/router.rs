// 多后端路由层

use super::StorageBackend;
use crate::{Result, VideoError};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// 将 `save_segment` 的返回值编码为：`router:{idx}:{url}`
const ROUTER_PREFIX: &str = "router:";

/// 根据 stream_id 做一致性分流的路由后端。
///
/// 说明：这是 M2.3 的最小可用形态，后续可以替换为：
/// - 按设备/组织/区域策略路由
/// - NAS/NVR/对象存储后端
/// - 一致性哈希环 + 副本
pub struct StorageRouter {
    backends: Vec<Arc<dyn StorageBackend>>,
}

impl StorageRouter {
    pub fn new(backends: Vec<Arc<dyn StorageBackend>>) -> Result<Self> {
        if backends.is_empty() {
            return Err(VideoError::Other("StorageRouter requires at least one backend".to_string()));
        }
        Ok(Self { backends })
    }

    fn select_backend_index(&self, stream_id: &str) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        stream_id.hash(&mut hasher);
        (hasher.finish() as usize) % self.backends.len()
    }

    fn encode_url(idx: usize, url: &str) -> String {
        format!("{}{}:{}", ROUTER_PREFIX, idx, url)
    }

    fn decode_url(encoded: &str) -> Result<(usize, &str)> {
        if !encoded.starts_with(ROUTER_PREFIX) {
            return Err(VideoError::Other(format!(
                "Invalid router url prefix: {}",
                encoded
            )));
        }

        let rest = &encoded[ROUTER_PREFIX.len()..];
        let Some((idx_str, url)) = rest.split_once(':') else {
            return Err(VideoError::Other(format!("Invalid router url format: {}", encoded)));
        };

        let idx: usize = idx_str.parse().map_err(|_| {
            VideoError::Other(format!("Invalid backend index in router url: {}", encoded))
        })?;

        Ok((idx, url))
    }
}

#[async_trait]
impl StorageBackend for StorageRouter {
    async fn save_segment(
        &self,
        stream_id: &str,
        timestamp: DateTime<Utc>,
        data: Bytes,
    ) -> Result<String> {
        let idx = self.select_backend_index(stream_id);
        let url = self.backends[idx]
            .save_segment(stream_id, timestamp, data)
            .await?;
        Ok(Self::encode_url(idx, &url))
    }

    async fn get_segment(&self, url: &str) -> Result<Bytes> {
        let (idx, inner_url) = Self::decode_url(url)?;
        let backend = self.backends.get(idx).ok_or_else(|| {
            VideoError::Other(format!("Backend index out of range: {}", idx))
        })?;
        backend.get_segment(inner_url).await
    }

    async fn list_segments(
        &self,
        stream_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<super::ObjectMetadata>> {
        let idx = self.select_backend_index(stream_id);
        self.backends[idx].list_segments(stream_id, start, end).await
    }

    async fn cleanup_expired(&self, before: DateTime<Utc>) -> Result<usize> {
        let mut deleted = 0usize;
        for backend in &self.backends {
            deleted += backend.cleanup_expired(before).await?;
        }
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::backend::LocalFsBackend;
    use crate::storage::StandaloneStorage;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_router_roundtrip() {
        let d1 = TempDir::new().unwrap();
        let d2 = TempDir::new().unwrap();

        let s1 = StandaloneStorage::new(d1.path().to_path_buf()).unwrap();
        let s2 = StandaloneStorage::new(d2.path().to_path_buf()).unwrap();

        let b1: Arc<dyn StorageBackend> = Arc::new(LocalFsBackend::new(s1));
        let b2: Arc<dyn StorageBackend> = Arc::new(LocalFsBackend::new(s2));

        let router = StorageRouter::new(vec![b1, b2]).unwrap();

        let stream_id = "cam001";
        let ts = Utc::now();
        let data = Bytes::from_static(b"hello");

        let url = router.save_segment(stream_id, ts, data.clone()).await.unwrap();
        assert!(url.starts_with(ROUTER_PREFIX));

        let got = router.get_segment(&url).await.unwrap();
        assert_eq!(got, data);
    }
}
