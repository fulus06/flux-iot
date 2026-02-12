// 单节点存储模式（极致轻量，40-80MB 内存）
use std::path::{Path, PathBuf};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use crate::Result;
use super::index::LightweightIndex;
use super::pipeline::WritePipeline;

/// 单节点存储（内存占用 < 80MB）
pub struct StandaloneStorage {
    /// 数据目录
    base_path: PathBuf,
    
    /// 轻量级索引（LRU 缓存）
    index: LightweightIndex,
    
    /// 写入流水线
    pipeline: WritePipeline,
    
    /// 保留天数
    retention_days: u32,
}

impl StandaloneStorage {
    pub fn new(base_path: PathBuf) -> Result<Self> {
        Self::with_config(base_path, 7, 8, 1000)
    }
    
    pub fn with_config(
        base_path: PathBuf,
        retention_days: u32,
        worker_count: usize,
        index_cache_size: usize,
    ) -> Result<Self> {
        std::fs::create_dir_all(&base_path)?;
        
        let index = LightweightIndex::new(index_cache_size);
        let pipeline = WritePipeline::new(worker_count);
        
        tracing::info!(
            "StandaloneStorage initialized at: {:?}, retention_days: {}, workers: {}",
            base_path, retention_days, worker_count
        );
        
        Ok(Self {
            base_path,
            index,
            pipeline,
            retention_days,
        })
    }
    
    /// 保存视频分片
    pub async fn put_object(
        &mut self,
        stream_id: &str,
        timestamp: DateTime<Utc>,
        data: Bytes,
    ) -> Result<String> {
        let object_key = ObjectKey {
            stream_id: stream_id.to_string(),
            timestamp,
            object_type: ObjectType::VideoSegment,
        };
        
        // 计算文件路径
        let file_path = self.get_object_path(&object_key);
        
        // 确保目录存在
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // 写入文件
        tokio::fs::write(&file_path, &data).await?;
        
        // 更新索引
        let metadata = ObjectMetadata {
            key: object_key.clone(),
            size: data.len() as u64,
            created_at: timestamp,
            path: file_path.to_string_lossy().to_string(),
        };
        
        self.index.put(object_key.to_string(), metadata);
        
        tracing::debug!(
            "Stored object: {} ({} bytes)",
            object_key.to_string(),
            data.len()
        );
        
        Ok(file_path.to_string_lossy().to_string())
    }
    
    /// 读取视频分片
    pub async fn get_object(&self, stream_id: &str, timestamp: DateTime<Utc>) -> Result<Bytes> {
        let object_key = ObjectKey {
            stream_id: stream_id.to_string(),
            timestamp,
            object_type: ObjectType::VideoSegment,
        };
        
        let file_path = self.get_object_path(&object_key);
        
        let data = tokio::fs::read(&file_path).await
            .map_err(|_| crate::VideoError::StorageError(format!("Object not found: {}", object_key.to_string())))?;
        
        Ok(Bytes::from(data))
    }
    
    /// 列出对象（扫描文件系统）
    pub async fn list_objects(
        &self,
        stream_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ObjectMetadata>> {
        let stream_dir = self.base_path.join(stream_id);
        
        if !stream_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut objects = Vec::new();
        self.scan_directory(&stream_dir, stream_id, start, end, &mut objects).await?;
        
        objects.sort_by_key(|o| o.created_at);
        Ok(objects)
    }
    
    /// 清理过期数据
    pub async fn cleanup_expired(&self, before: DateTime<Utc>) -> Result<usize> {
        let mut deleted = 0;
        
        let mut entries = tokio::fs::read_dir(&self.base_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                deleted += self.cleanup_stream_dir(&entry.path(), before).await?;
            }
        }
        
        tracing::info!("Cleaned up {} expired objects", deleted);
        Ok(deleted)
    }
    
    /// 获取数据目录
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }
    
    // 辅助方法
    fn get_object_path(&self, key: &ObjectKey) -> PathBuf {
        // 目录结构：base_path/stream_id/YYYY-MM-DD/HH/timestamp.ext
        let date_path = key.timestamp.format("%Y-%m-%d/%H").to_string();
        let filename = format!(
            "{}.{}",
            key.timestamp.timestamp(),
            match key.object_type {
                ObjectType::VideoSegment => "mp4",
                ObjectType::Keyframe => "jpg",
                ObjectType::Metadata => "json",
            }
        );
        
        self.base_path
            .join(&key.stream_id)
            .join(date_path)
            .join(filename)
    }
    
    fn scan_directory<'a>(
        &'a self,
        dir: &'a Path,
        stream_id: &'a str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        objects: &'a mut Vec<ObjectMetadata>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = tokio::fs::read_dir(dir).await?;
            
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                
                if path.is_dir() {
                    self.scan_directory(&path, stream_id, start, end, objects).await?;
                } else if let Some(timestamp) = self.parse_timestamp_from_path(&path) {
                    if timestamp >= start && timestamp <= end {
                        let metadata = tokio::fs::metadata(&path).await?;
                        objects.push(ObjectMetadata {
                            key: ObjectKey {
                                stream_id: stream_id.to_string(),
                                timestamp,
                                object_type: ObjectType::VideoSegment,
                            },
                            size: metadata.len(),
                            created_at: timestamp,
                            path: path.to_string_lossy().to_string(),
                        });
                    }
                }
            }
            
            Ok(())
        })
    }
    
    fn parse_timestamp_from_path(&self, path: &Path) -> Option<DateTime<Utc>> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<i64>().ok())
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
    }
    
    fn cleanup_stream_dir<'a>(
        &'a self,
        dir: &'a Path,
        before: DateTime<Utc>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize>> + Send + 'a>> {
        Box::pin(async move {
            let mut deleted = 0;
            let mut entries = tokio::fs::read_dir(dir).await?;
            
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                
                if path.is_dir() {
                    deleted += self.cleanup_stream_dir(&path, before).await?;
                } else if let Some(timestamp) = self.parse_timestamp_from_path(&path) {
                    if timestamp < before {
                        tokio::fs::remove_file(&path).await?;
                        deleted += 1;
                    }
                }
            }
            
            Ok(deleted)
        })
    }
}

/// 对象键
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ObjectKey {
    pub stream_id: String,
    pub timestamp: DateTime<Utc>,
    pub object_type: ObjectType,
}

impl ObjectKey {
    pub fn to_string(&self) -> String {
        format!(
            "{}/{}/{}",
            self.stream_id,
            self.timestamp.timestamp(),
            match self.object_type {
                ObjectType::VideoSegment => "segment",
                ObjectType::Keyframe => "keyframe",
                ObjectType::Metadata => "metadata",
            }
        )
    }
}

/// 对象类型
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ObjectType {
    VideoSegment,
    Keyframe,
    Metadata,
}

/// 对象元数据
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    pub key: ObjectKey,
    pub size: u64,
    pub created_at: DateTime<Utc>,
    pub path: String,
}
