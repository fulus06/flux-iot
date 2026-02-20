use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info};

use crate::manager::StorageManager;

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
}

impl LocalSegmentStorage {
    /// 创建新的本地分片存储
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            storage_manager: None,
            base_dir,
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
