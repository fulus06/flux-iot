use crate::error::{MediaError, Result};
use crate::types::{StreamId, VideoSample};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

/// 媒体存储配置
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub root_dir: PathBuf,
    pub retention_days: u32,
    pub segment_duration_secs: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            root_dir: PathBuf::from("/tmp/flux-media"),
            retention_days: 7,
            segment_duration_secs: 60,
        }
    }
}

/// 协议无关的媒体存储接口
#[async_trait]
pub trait MediaStorage: Send + Sync {
    /// 存储对象（通用接口）
    async fn put_object(
        &mut self,
        stream_id: &StreamId,
        timestamp: DateTime<Utc>,
        data: Bytes,
    ) -> Result<()>;

    /// 获取对象
    async fn get_object(&self, stream_id: &StreamId, timestamp: DateTime<Utc>)
        -> Result<Option<Bytes>>;

    /// 列出流的所有对象
    async fn list_objects(
        &self,
        stream_id: &StreamId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ObjectInfo>>;

    /// 清理过期数据
    async fn cleanup(&mut self, before: DateTime<Utc>) -> Result<usize>;

    /// 存储视频样本（高级接口）
    async fn put_sample(&mut self, stream_id: &StreamId, sample: VideoSample) -> Result<()> {
        self.put_object(stream_id, sample.timestamp, sample.data)
            .await
    }
}

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub stream_id: StreamId,
    pub timestamp: DateTime<Utc>,
    pub size: usize,
    pub path: PathBuf,
}

/// 基于文件系统的存储实现
pub mod filesystem {
    use super::*;
    use std::fs;
    use tokio::fs as async_fs;

    pub struct FileSystemStorage {
        config: StorageConfig,
    }

    impl FileSystemStorage {
        pub fn new(config: StorageConfig) -> Result<Self> {
            fs::create_dir_all(&config.root_dir).map_err(|e| {
                MediaError::Storage(format!("Failed to create root dir: {}", e))
            })?;
            Ok(Self { config })
        }

        fn object_path(&self, stream_id: &StreamId, timestamp: DateTime<Utc>) -> PathBuf {
            let ts_millis = timestamp.timestamp_millis();
            self.config
                .root_dir
                .join(stream_id.as_str())
                .join("segments")
                .join(format!("{}.bin", ts_millis))
        }

        fn stream_dir(&self, stream_id: &StreamId) -> PathBuf {
            self.config.root_dir.join(stream_id.as_str())
        }
    }

    #[async_trait]
    impl MediaStorage for FileSystemStorage {
        async fn put_object(
            &mut self,
            stream_id: &StreamId,
            timestamp: DateTime<Utc>,
            data: Bytes,
        ) -> Result<()> {
            let path = self.object_path(stream_id, timestamp);
            if let Some(parent) = path.parent() {
                async_fs::create_dir_all(parent).await?;
            }
            async_fs::write(&path, &data).await?;
            Ok(())
        }

        async fn get_object(
            &self,
            stream_id: &StreamId,
            timestamp: DateTime<Utc>,
        ) -> Result<Option<Bytes>> {
            let path = self.object_path(stream_id, timestamp);
            if !path.exists() {
                return Ok(None);
            }
            let data = async_fs::read(&path).await?;
            Ok(Some(Bytes::from(data)))
        }

        async fn list_objects(
            &self,
            stream_id: &StreamId,
            start: DateTime<Utc>,
            end: DateTime<Utc>,
        ) -> Result<Vec<ObjectInfo>> {
            let segments_dir = self.stream_dir(stream_id).join("segments");
            if !segments_dir.exists() {
                return Ok(Vec::new());
            }

            let mut objects = Vec::new();
            let mut entries = async_fs::read_dir(&segments_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(ts_millis) = name.parse::<i64>() {
                        let timestamp = DateTime::from_timestamp_millis(ts_millis)
                            .unwrap_or_else(|| Utc::now());
                        if timestamp >= start && timestamp <= end {
                            let metadata = entry.metadata().await?;
                            objects.push(ObjectInfo {
                                stream_id: stream_id.clone(),
                                timestamp,
                                size: metadata.len() as usize,
                                path: path.clone(),
                            });
                        }
                    }
                }
            }

            objects.sort_by_key(|o| o.timestamp);
            Ok(objects)
        }

        async fn cleanup(&mut self, before: DateTime<Utc>) -> Result<usize> {
            let mut count = 0;
            let mut entries = async_fs::read_dir(&self.config.root_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let _stream_id = StreamId::from_string(
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string(),
                );

                let segments_dir = path.join("segments");
                if !segments_dir.exists() {
                    continue;
                }

                let mut segment_entries = async_fs::read_dir(&segments_dir).await?;
                while let Some(seg_entry) = segment_entries.next_entry().await? {
                    let seg_path = seg_entry.path();
                    if let Some(name) = seg_path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(ts_millis) = name.parse::<i64>() {
                            let timestamp = DateTime::from_timestamp_millis(ts_millis)
                                .unwrap_or_else(|| Utc::now());
                            if timestamp < before {
                                async_fs::remove_file(&seg_path).await?;
                                count += 1;
                            }
                        }
                    }
                }
            }

            Ok(count)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::tempdir;

        #[tokio::test]
        async fn test_filesystem_storage_put_get() {
            let temp_dir = tempdir().unwrap();
            let config = StorageConfig {
                root_dir: temp_dir.path().to_path_buf(),
                retention_days: 7,
                segment_duration_secs: 60,
            };

            let mut storage = FileSystemStorage::new(config).unwrap();
            let stream_id = StreamId::new("test", "stream1");
            let timestamp = Utc::now();
            let data = Bytes::from("test data");

            storage
                .put_object(&stream_id, timestamp, data.clone())
                .await
                .unwrap();

            let retrieved = storage.get_object(&stream_id, timestamp).await.unwrap();
            assert_eq!(retrieved, Some(data));
        }

        #[tokio::test]
        async fn test_filesystem_storage_list() {
            let temp_dir = tempdir().unwrap();
            let config = StorageConfig {
                root_dir: temp_dir.path().to_path_buf(),
                retention_days: 7,
                segment_duration_secs: 60,
            };

            let mut storage = FileSystemStorage::new(config).unwrap();
            let stream_id = StreamId::new("test", "stream1");

            let now = Utc::now();
            for i in 0..5 {
                let ts = now + chrono::Duration::seconds(i);
                storage
                    .put_object(&stream_id, ts, Bytes::from(format!("data{}", i)))
                    .await
                    .unwrap();
            }

            // 等待文件系统同步
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            let objects = storage
                .list_objects(&stream_id, now, now + chrono::Duration::seconds(10))
                .await
                .unwrap();

            assert!(objects.len() >= 4, "Expected at least 4 objects, got {}", objects.len());
        }
    }
}
