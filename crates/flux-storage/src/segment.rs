use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::manager::StorageManager;

/// 分片元数据（类似 OSS 对象元数据）
/// 
/// 通用的 key-value 元数据结构，由调用方自定义内容
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SegmentMetadata {
    /// 自定义元数据（key-value 对）
    /// 
    /// 示例：
    /// - "stream_id": "app/stream1"
    /// - "segment_id": "1"
    /// - "start_time": "2026-02-23T15:00:00Z"
    /// - "duration": "10.0"
    /// - "size": "1024000"
    /// - "has_keyframe": "true"
    /// - "codec": "h264"
    /// - "resolution": "1920x1080"
    pub metadata: HashMap<String, String>,
}

impl SegmentMetadata {
    /// 创建新的元数据
    pub fn new() -> Self {
        Self {
            metadata: HashMap::new(),
        }
    }
    
    /// 设置元数据
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// 获取元数据
    pub fn get(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
    
    /// 批量设置元数据
    pub fn set_many(&mut self, pairs: Vec<(String, String)>) -> &mut Self {
        for (k, v) in pairs {
            self.metadata.insert(k, v);
        }
        self
    }
}

/// 分片存储抽象 trait
#[async_trait]
pub trait SegmentStorage: Send + Sync {
    /// 保存分片
    /// 
    /// # 参数
    /// - `stream_id`: 流 ID
    /// - `segment_id`: 分片序号
    /// - `data`: 分片数据
    /// 
    /// # 返回
    /// 分片文件名
    async fn save_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
        data: &[u8],
    ) -> Result<String>;
    
    /// 保存分片（带元数据）
    /// 
    /// # 参数
    /// - `stream_id`: 流 ID
    /// - `segment_id`: 分片序号
    /// - `metadata`: 自定义元数据
    /// - `data`: 分片数据
    async fn save_segment_with_metadata(
        &self,
        stream_id: &str,
        segment_id: u64,
        metadata: SegmentMetadata,
        data: &[u8],
    ) -> Result<String> {
        // 默认实现：只保存数据，忽略元数据
        self.save_segment(stream_id, segment_id, data).await
    }
    
    /// 加载分片
    async fn load_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<Bytes>;
    
    /// 删除分片
    async fn delete_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<()>;
    
    /// 列出流的所有分片
    async fn list_segments(&self, stream_id: &str) -> Result<Vec<u64>>;
    
    /// 获取分片元数据
    async fn get_segment_metadata(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<SegmentMetadata> {
        // 默认实现：返回基础元数据
        Err(anyhow!("Metadata not supported"))
    }
    
    /// 查询元数据
    /// 
    /// # 参数
    /// - `stream_id`: 流 ID
    /// - `filter`: 过滤条件（key-value 对，所有条件必须匹配）
    /// 
    /// # 返回
    /// 返回 (segment_id, metadata) 列表
    async fn query_metadata(
        &self,
        stream_id: &str,
        filter: HashMap<String, String>,
    ) -> Result<Vec<(u64, SegmentMetadata)>> {
        // 默认实现：不支持
        Err(anyhow!("Metadata query not supported"))
    }
    
    /// 清理过期分片
    /// 
    /// # 参数
    /// - `stream_id`: 流 ID
    /// - `keep_count`: 保留的分片数量
    async fn cleanup_old_segments(
        &self,
        stream_id: &str,
        keep_count: usize,
    ) -> Result<usize>;
}

/// 分片存储实现（使用 StorageManager）
pub struct SegmentStorageImpl {
    /// 存储管理器
    storage_manager: Arc<StorageManager>,
}

impl SegmentStorageImpl {
    /// 创建新的分片存储
    pub fn new(storage_manager: Arc<StorageManager>) -> Self {
        Self { storage_manager }
    }
    
    /// 构造分片路径（业务逻辑）
    fn build_segment_path(&self, stream_id: &str, segment_id: u64) -> String {
        format!("hls/{}/segment_{}.ts", stream_id, segment_id)
    }
    
    /// 解析分片 ID
    fn parse_segment_id(&self, filename: &str) -> Option<u64> {
        if filename.starts_with("segment_") && filename.ends_with(".ts") {
            let id_str = &filename[8..filename.len() - 3];
            id_str.parse::<u64>().ok()
        } else {
            None
        }
    }
}

/// 本地文件系统分片存储（兼容旧接口）
pub struct LocalSegmentStorage {
    /// 存储管理器（用于选择存储池）
    storage_manager: Option<Arc<StorageManager>>,
    
    /// 基础目录（当没有 StorageManager 时使用）
    base_dir: PathBuf,
    
    /// 元数据索引（内存缓存，类似 OSS）
    /// Key: "stream_id:segment_id"
    /// Value: SegmentMetadata
    metadata_index: Arc<RwLock<HashMap<String, SegmentMetadata>>>,
    
    /// PostgreSQL 元数据后端（可选，持久化）
    #[cfg(feature = "postgres")]
    pg_backend: Option<Arc<crate::metadata_pg::PostgresMetadataBackend>>,
}

impl LocalSegmentStorage {
    /// 创建新的本地分片存储
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            storage_manager: None,
            base_dir,
            metadata_index: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "postgres")]
            pg_backend: None,
        }
    }
    
    /// 创建带存储管理器的本地分片存储
    pub fn with_storage_manager(
        storage_manager: Arc<StorageManager>,
        base_dir: PathBuf,
    ) -> Self {
        Self {
            storage_manager: Some(storage_manager),
            base_dir,
            metadata_index: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "postgres")]
            pg_backend: None,
        }
    }
    
    /// 创建带 PostgreSQL 后端的本地分片存储（混合模式）
    #[cfg(feature = "postgres")]
    pub fn with_postgres(
        base_dir: PathBuf,
        pg_backend: Option<Arc<crate::metadata_pg::PostgresMetadataBackend>>,
    ) -> Self {
        Self {
            storage_manager: None,
            base_dir,
            metadata_index: Arc::new(RwLock::new(HashMap::new())),
            pg_backend,
        }
    }
    
    /// 创建带存储管理器和 PostgreSQL 后端的本地分片存储（完整混合模式）
    #[cfg(feature = "postgres")]
    pub fn with_storage_manager_and_postgres(
        storage_manager: Arc<StorageManager>,
        base_dir: PathBuf,
        pg_backend: Option<Arc<crate::metadata_pg::PostgresMetadataBackend>>,
    ) -> Self {
        Self {
            storage_manager: Some(storage_manager),
            base_dir,
            metadata_index: Arc::new(RwLock::new(HashMap::new())),
            pg_backend,
        }
    }
    
    /// 获取分片目录
    async fn get_segment_dir(&self, stream_id: &str, data_size: u64) -> Result<PathBuf> {
        let base = if let Some(ref manager) = self.storage_manager {
            // 使用存储管理器选择最佳存储池
            manager.select_pool(data_size).await.unwrap_or_else(|_| {
                debug!("Failed to select pool, using base_dir");
                self.base_dir.clone()
            })
        } else {
            self.base_dir.clone()
        };
        
        Ok(base.join("hls").join(stream_id))
    }
    
    /// 获取分片文件路径
    async fn get_segment_path(&self, stream_id: &str, segment_id: u64, data_size: u64) -> Result<PathBuf> {
        let dir = self.get_segment_dir(stream_id, data_size).await?;
        let filename = format!("segment_{}.ts", segment_id);
        Ok(dir.join(filename))
    }
}

#[async_trait]
impl SegmentStorage for SegmentStorageImpl {
    async fn save_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
        data: &[u8],
    ) -> Result<String> {
        // 1. 构造路径（业务逻辑）
        let path = self.build_segment_path(stream_id, segment_id);
        
        // 2. 使用 StorageManager 选择池并写入
        let pool_name = self.storage_manager
            .write_with_selection(&path, data)
            .await?;
        
        info!(
            stream_id = %stream_id,
            segment_id = segment_id,
            pool = %pool_name,
            size = data.len(),
            "Segment saved via StorageManager"
        );
        
        Ok(format!("segment_{}.ts", segment_id))
    }
    
    async fn load_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<Bytes> {
        let path = self.build_segment_path(stream_id, segment_id);
        
        // 尝试从所有池中读取
        let pools = self.storage_manager.get_pools().await;
        
        for (pool_name, _, _, _) in pools {
            if let Ok(data) = self.storage_manager.read_from_pool(&pool_name, &path).await {
                debug!(
                    stream_id = %stream_id,
                    segment_id = segment_id,
                    pool = %pool_name,
                    "Segment loaded"
                );
                return Ok(data);
            }
        }
        
        Err(anyhow!("Segment not found: {}/{}", stream_id, segment_id))
    }
    
    async fn delete_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<()> {
        let path = self.build_segment_path(stream_id, segment_id);
        
        // 使用 StorageManager 从所有池中删除
        self.storage_manager.delete_from_any_pool(&path).await?;
        
        debug!(
            stream_id = %stream_id,
            segment_id = segment_id,
            "Segment deleted"
        );
        
        Ok(())
    }
    
    async fn list_segments(&self, stream_id: &str) -> Result<Vec<u64>> {
        let prefix = format!("hls/{}/", stream_id);
        
        // 从所有池中列出文件
        let files = self.storage_manager.list_from_all_pools(&prefix).await?;
        
        let mut all_segments = std::collections::HashSet::new();
        
        for file in files {
            if let Some(id) = self.parse_segment_id(&file) {
                all_segments.insert(id);
            }
        }
        
        let mut segments: Vec<u64> = all_segments.into_iter().collect();
        segments.sort_unstable();
        
        Ok(segments)
    }
    
    async fn cleanup_old_segments(
        &self,
        stream_id: &str,
        keep_count: usize,
    ) -> Result<usize> {
        let segments = self.list_segments(stream_id).await?;
        
        if segments.len() <= keep_count {
            return Ok(0);
        }
        
        let to_delete = &segments[..segments.len() - keep_count];
        let mut deleted = 0;
        
        for &segment_id in to_delete {
            if self.delete_segment(stream_id, segment_id).await.is_ok() {
                deleted += 1;
            }
        }
        
        info!(
            stream_id = %stream_id,
            deleted = deleted,
            kept = keep_count,
            "Old segments cleaned up"
        );
        
        Ok(deleted)
    }
}

#[async_trait]
impl SegmentStorage for LocalSegmentStorage {
    async fn save_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
        data: &[u8],
    ) -> Result<String> {
        let segment_dir = self.get_segment_dir(stream_id, data.len() as u64).await?;
        
        // 创建目录
        if let Err(e) = fs::create_dir_all(&segment_dir).await {
            error!("Failed to create segment directory: {}", e);
            return Err(e.into());
        }
        
        // 构造文件名和路径
        let filename = format!("segment_{}.ts", segment_id);
        let segment_path = segment_dir.join(&filename);
        
        // 写入文件
        match fs::File::create(&segment_path).await {
            Ok(mut file) => {
                if let Err(e) = file.write_all(data).await {
                    error!("Failed to write segment data: {}", e);
                    return Err(e.into());
                }
                
                info!(
                    stream_id = %stream_id,
                    segment_id = segment_id,
                    size = data.len(),
                    path = ?segment_path,
                    "Segment saved"
                );
                
                Ok(filename)
            }
            Err(e) => {
                error!("Failed to create segment file: {}", e);
                Err(e.into())
            }
        }
    }
    
    async fn load_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<Bytes> {
        // 尝试从可能的存储池中加载
        let segment_path = self.get_segment_path(stream_id, segment_id, 0).await?;
        
        match fs::read(&segment_path).await {
            Ok(data) => {
                debug!(
                    stream_id = %stream_id,
                    segment_id = segment_id,
                    size = data.len(),
                    "Segment loaded"
                );
                Ok(Bytes::from(data))
            }
            Err(e) => {
                error!("Failed to read segment: {}", e);
                Err(e.into())
            }
        }
    }
    
    async fn delete_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<()> {
        let segment_path = self.get_segment_path(stream_id, segment_id, 0).await?;
        
        match fs::remove_file(&segment_path).await {
            Ok(_) => {
                debug!(
                    stream_id = %stream_id,
                    segment_id = segment_id,
                    "Segment deleted"
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to delete segment: {}", e);
                Err(e.into())
            }
        }
    }
    
    async fn list_segments(&self, stream_id: &str) -> Result<Vec<u64>> {
        let segment_dir = self.get_segment_dir(stream_id, 0).await?;
        
        let mut segments = Vec::new();
        
        match fs::read_dir(&segment_dir).await {
            Ok(mut entries) => {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Some(filename) = entry.file_name().to_str() {
                        // 解析文件名：segment_{id}.ts
                        if filename.starts_with("segment_") && filename.ends_with(".ts") {
                            let id_str = &filename[8..filename.len() - 3];
                            if let Ok(id) = id_str.parse::<u64>() {
                                segments.push(id);
                            }
                        }
                    }
                }
                
                // 按 ID 排序
                segments.sort_unstable();
                
                Ok(segments)
            }
            Err(e) => {
                error!("Failed to list segments: {}", e);
                Err(e.into())
            }
        }
    }
    
    async fn cleanup_old_segments(
        &self,
        stream_id: &str,
        keep_count: usize,
    ) -> Result<usize> {
        let segments = self.list_segments(stream_id).await?;
        
        if segments.len() <= keep_count {
            return Ok(0);
        }
        
        let to_delete = &segments[..segments.len() - keep_count];
        let mut deleted = 0;
        
        for &segment_id in to_delete {
            if self.delete_segment(stream_id, segment_id).await.is_ok() {
                deleted += 1;
            }
        }
        
        info!(
            stream_id = %stream_id,
            deleted = deleted,
            kept = keep_count,
            "Old segments cleaned up"
        );
        
        Ok(deleted)
    }
    
    async fn save_segment_with_metadata(
        &self,
        stream_id: &str,
        segment_id: u64,
        metadata: SegmentMetadata,
        data: &[u8],
    ) -> Result<String> {
        // 1. 保存数据
        let filename = self.save_segment(stream_id, segment_id, data).await?;
        
        // 2. 保存到内存索引（缓存）
        {
            let mut index = self.metadata_index.write().await;
            let key = format!("{}:{}", stream_id, segment_id);
            index.insert(key, metadata.clone());
        }
        
        // 3. 异步保存到 PostgreSQL（write-through）
        #[cfg(feature = "postgres")]
        if let Some(ref pg) = self.pg_backend {
            let pg = pg.clone();
            let stream_id = stream_id.to_string();
            let metadata = metadata.clone();
            
            tokio::spawn(async move {
                if let Err(e) = pg.save_metadata(&stream_id, segment_id, &metadata).await {
                    error!("Failed to save metadata to PostgreSQL: {}", e);
                }
            });
        }
        
        debug!(
            stream_id = %stream_id,
            segment_id = segment_id,
            has_pg = cfg!(feature = "postgres"),
            "Metadata saved (hybrid mode)"
        );
        
        Ok(filename)
    }
    
    async fn get_segment_metadata(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<SegmentMetadata> {
        let key = format!("{}:{}", stream_id, segment_id);
        
        // 1. 尝试从内存缓存读取
        {
            let index = self.metadata_index.read().await;
            if let Some(metadata) = index.get(&key) {
                debug!(stream_id = %stream_id, segment_id = segment_id, "Metadata hit in cache");
                return Ok(metadata.clone());
            }
        }
        
        // 2. 缓存未命中，从 PostgreSQL 读取
        #[cfg(feature = "postgres")]
        if let Some(ref pg) = self.pg_backend {
            match pg.get_metadata(stream_id, segment_id).await {
                Ok(metadata) => {
                    // 更新缓存
                    let mut index = self.metadata_index.write().await;
                    index.insert(key, metadata.clone());
                    
                    debug!(stream_id = %stream_id, segment_id = segment_id, "Metadata loaded from PostgreSQL and cached");
                    return Ok(metadata);
                }
                Err(e) => {
                    debug!("PostgreSQL metadata not found: {}", e);
                }
            }
        }
        
        Err(anyhow!("Metadata not found: {}/{}", stream_id, segment_id))
    }
    
    async fn query_metadata(
        &self,
        stream_id: &str,
        filter: HashMap<String, String>,
    ) -> Result<Vec<(u64, SegmentMetadata)>> {
        // 优先使用 PostgreSQL 查询（更强大的查询能力）
        #[cfg(feature = "postgres")]
        if let Some(ref pg) = self.pg_backend {
            match pg.query_metadata(stream_id, filter.clone()).await {
                Ok(results) => {
                    debug!(
                        stream_id = %stream_id,
                        count = results.len(),
                        "Metadata queried from PostgreSQL"
                    );
                    return Ok(results);
                }
                Err(e) => {
                    debug!("PostgreSQL query failed, falling back to memory: {}", e);
                }
            }
        }
        
        // 回退到内存查询
        let index = self.metadata_index.read().await;
        let prefix = format!("{}:", stream_id);
        let mut results = Vec::new();
        for (key, metadata) in index.iter() {
            if !key.starts_with(&prefix) { continue; }
            let matches = filter.iter().all(|(k, v)| metadata.get(k).map(|mv| mv == v).unwrap_or(false));
            if matches {
                if let Some(id_str) = key.strip_prefix(&prefix) {
                    if let Ok(segment_id) = id_str.parse::<u64>() {
                        results.push((segment_id, metadata.clone()));
                    }
                }
            }
        }
        results.sort_by_key(|(id, _)| *id);
        debug!(stream_id = %stream_id, count = results.len(), "Metadata queried from memory");
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_segment_storage_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalSegmentStorage::new(temp_dir.path().to_path_buf());
        
        let stream_id = "test/stream";
        let segment_id = 1;
        let data = b"test segment data";
        
        // 保存分片
        let filename = storage.save_segment(stream_id, segment_id, data).await.unwrap();
        assert_eq!(filename, "segment_1.ts");
        
        // 加载分片
        let loaded = storage.load_segment(stream_id, segment_id).await.unwrap();
        assert_eq!(&loaded[..], data);
    }

    #[tokio::test]
    async fn test_local_segment_storage_list() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalSegmentStorage::new(temp_dir.path().to_path_buf());
        
        let stream_id = "test/stream";
        
        // 保存多个分片
        for i in 1..=5 {
            storage.save_segment(stream_id, i, b"data").await.unwrap();
        }
        
        // 列出分片
        let segments = storage.list_segments(stream_id).await.unwrap();
        assert_eq!(segments, vec![1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn test_local_segment_storage_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalSegmentStorage::new(temp_dir.path().to_path_buf());
        
        let stream_id = "test/stream";
        
        // 保存 10 个分片
        for i in 1..=10 {
            storage.save_segment(stream_id, i, b"data").await.unwrap();
        }
        
        // 清理，只保留最新的 5 个
        let deleted = storage.cleanup_old_segments(stream_id, 5).await.unwrap();
        assert_eq!(deleted, 5);
        
        // 验证剩余分片
        let segments = storage.list_segments(stream_id).await.unwrap();
        assert_eq!(segments, vec![6, 7, 8, 9, 10]);
    }

    #[tokio::test]
    async fn test_local_segment_storage_delete() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalSegmentStorage::new(temp_dir.path().to_path_buf());
        
        let stream_id = "test/stream";
        let segment_id = 1;
        
        // 保存分片
        storage.save_segment(stream_id, segment_id, b"data").await.unwrap();
        
        // 删除分片
        storage.delete_segment(stream_id, segment_id).await.unwrap();
        
        // 验证已删除
        let result = storage.load_segment(stream_id, segment_id).await;
        assert!(result.is_err());
    }
}
