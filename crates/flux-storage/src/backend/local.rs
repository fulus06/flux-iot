use super::{BackendStats, FileMetadata, StorageBackend};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::fs;
use tracing::{debug, error};

/// 本地文件系统存储后端
/// 
/// # 性能优化
/// - 使用 tokio 异步 I/O
/// - 零拷贝读取（mmap 可选）
/// - 批量操作优化
/// - 统计信息收集
pub struct LocalBackend {
    /// 基础目录
    base_dir: PathBuf,
    
    /// 统计信息
    stats: Arc<LocalBackendStats>,
}

/// 本地后端统计信息
struct LocalBackendStats {
    read_count: AtomicU64,
    write_count: AtomicU64,
    delete_count: AtomicU64,
    bytes_read: AtomicU64,
    bytes_written: AtomicU64,
}

impl LocalBackend {
    /// 创建新的本地后端
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            stats: Arc::new(LocalBackendStats {
                read_count: AtomicU64::new(0),
                write_count: AtomicU64::new(0),
                delete_count: AtomicU64::new(0),
                bytes_read: AtomicU64::new(0),
                bytes_written: AtomicU64::new(0),
            }),
        }
    }
    
    /// 解析完整路径
    fn resolve_path(&self, path: &str) -> PathBuf {
        // 移除开头的斜杠，避免路径问题
        let clean_path = path.trim_start_matches('/');
        self.base_dir.join(clean_path)
    }
    
    /// 确保父目录存在
    async fn ensure_parent_dir(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for LocalBackend {
    async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let full_path = self.resolve_path(path);
        
        // 确保父目录存在
        self.ensure_parent_dir(&full_path).await?;
        
        // 异步写入文件
        // 性能优化：使用 write_all 一次性写入
        match fs::write(&full_path, data).await {
            Ok(_) => {
                // 更新统计
                self.stats.write_count.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_written.fetch_add(data.len() as u64, Ordering::Relaxed);
                
                debug!(
                    path = %path,
                    size = data.len(),
                    "File written to local storage"
                );
                Ok(())
            }
            Err(e) => {
                error!(path = %path, error = %e, "Failed to write file");
                Err(anyhow!("Failed to write file: {}", e))
            }
        }
    }
    
    async fn write_batch(&self, items: Vec<(String, Vec<u8>)>) -> Result<Vec<Result<()>>> {
        // 性能优化：并发写入
        let mut tasks = Vec::with_capacity(items.len());
        
        for (path, data) in items {
            let backend = self.clone();
            tasks.push(tokio::spawn(async move {
                backend.write(&path, &data).await
            }));
        }
        
        // 等待所有任务完成
        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(anyhow!("Task failed: {}", e))),
            }
        }
        
        Ok(results)
    }
    
    async fn read(&self, path: &str) -> Result<Bytes> {
        let full_path = self.resolve_path(path);
        
        // 异步读取文件
        // 性能优化：直接读取为 Vec<u8>，然后转换为 Bytes（零拷贝）
        match fs::read(&full_path).await {
            Ok(data) => {
                let size = data.len();
                
                // 更新统计
                self.stats.read_count.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_read.fetch_add(size as u64, Ordering::Relaxed);
                
                debug!(
                    path = %path,
                    size = size,
                    "File read from local storage"
                );
                
                Ok(Bytes::from(data))
            }
            Err(e) => {
                error!(path = %path, error = %e, "Failed to read file");
                Err(anyhow!("Failed to read file: {}", e))
            }
        }
    }
    
    async fn read_range(&self, path: &str, start: u64, length: u64) -> Result<Bytes> {
        let full_path = self.resolve_path(path);
        
        // 性能优化：使用 tokio::fs::File + seek + read
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        
        let mut file = fs::File::open(&full_path).await?;
        file.seek(std::io::SeekFrom::Start(start)).await?;
        
        let mut buffer = vec![0u8; length as usize];
        let n = file.read(&mut buffer).await?;
        buffer.truncate(n);
        
        self.stats.read_count.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_read.fetch_add(n as u64, Ordering::Relaxed);
        
        Ok(Bytes::from(buffer))
    }
    
    async fn delete(&self, path: &str) -> Result<()> {
        let full_path = self.resolve_path(path);
        
        match fs::remove_file(&full_path).await {
            Ok(_) => {
                self.stats.delete_count.fetch_add(1, Ordering::Relaxed);
                debug!(path = %path, "File deleted from local storage");
                Ok(())
            }
            Err(e) => {
                error!(path = %path, error = %e, "Failed to delete file");
                Err(anyhow!("Failed to delete file: {}", e))
            }
        }
    }
    
    async fn delete_batch(&self, paths: Vec<String>) -> Result<Vec<Result<()>>> {
        // 性能优化：并发删除
        let mut tasks = Vec::with_capacity(paths.len());
        
        for path in paths {
            let backend = self.clone();
            tasks.push(tokio::spawn(async move {
                backend.delete(&path).await
            }));
        }
        
        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(anyhow!("Task failed: {}", e))),
            }
        }
        
        Ok(results)
    }
    
    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let dir = self.resolve_path(prefix);
        
        if !dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut entries = Vec::new();
        let mut read_dir = fs::read_dir(&dir).await?;
        
        while let Some(entry) = read_dir.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                entries.push(name.to_string());
            }
        }
        
        // 排序以保证一致性
        entries.sort();
        
        Ok(entries)
    }
    
    async fn exists(&self, path: &str) -> Result<bool> {
        let full_path = self.resolve_path(path);
        Ok(full_path.exists())
    }
    
    async fn metadata(&self, path: &str) -> Result<FileMetadata> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await?;
        
        let modified_time = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        
        Ok(FileMetadata {
            size: metadata.len(),
            modified_time,
            content_type: None, // 本地文件系统不存储 content-type
            etag: None,
        })
    }
    
    fn backend_type(&self) -> &str {
        "local"
    }
    
    async fn stats(&self) -> BackendStats {
        BackendStats {
            read_count: self.stats.read_count.load(Ordering::Relaxed),
            write_count: self.stats.write_count.load(Ordering::Relaxed),
            delete_count: self.stats.delete_count.load(Ordering::Relaxed),
            bytes_read: self.stats.bytes_read.load(Ordering::Relaxed),
            bytes_written: self.stats.bytes_written.load(Ordering::Relaxed),
            avg_read_latency_ms: 0.0,  // TODO: 实现延迟统计
            avg_write_latency_ms: 0.0,
            cache_hit_rate: 0.0,
        }
    }
}

// 实现 Clone 以支持并发操作
impl Clone for LocalBackend {
    fn clone(&self) -> Self {
        Self {
            base_dir: self.base_dir.clone(),
            stats: Arc::clone(&self.stats),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_backend_write_read() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalBackend::new(temp_dir.path().to_path_buf());
        
        let path = "test/file.txt";
        let data = b"Hello, World!";
        
        // 写入
        backend.write(path, data).await.unwrap();
        
        // 读取
        let read_data = backend.read(path).await.unwrap();
        assert_eq!(&read_data[..], data);
        
        // 统计
        let stats = backend.stats().await;
        assert_eq!(stats.write_count, 1);
        assert_eq!(stats.read_count, 1);
        assert_eq!(stats.bytes_written, data.len() as u64);
    }

    #[tokio::test]
    async fn test_local_backend_batch_write() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalBackend::new(temp_dir.path().to_path_buf());
        
        let items = vec![
            ("file1.txt".to_string(), b"data1".to_vec()),
            ("file2.txt".to_string(), b"data2".to_vec()),
            ("file3.txt".to_string(), b"data3".to_vec()),
        ];
        
        let results = backend.write_batch(items).await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));
        
        let stats = backend.stats().await;
        assert_eq!(stats.write_count, 3);
    }

    #[tokio::test]
    async fn test_local_backend_read_range() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalBackend::new(temp_dir.path().to_path_buf());
        
        let path = "test.txt";
        let data = b"0123456789";
        
        backend.write(path, data).await.unwrap();
        
        // 读取范围 [2, 5)
        let range_data = backend.read_range(path, 2, 3).await.unwrap();
        assert_eq!(&range_data[..], b"234");
    }

    #[tokio::test]
    async fn test_local_backend_list() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalBackend::new(temp_dir.path().to_path_buf());
        
        // 写入多个文件
        backend.write("dir/file1.txt", b"data1").await.unwrap();
        backend.write("dir/file2.txt", b"data2").await.unwrap();
        backend.write("dir/file3.txt", b"data3").await.unwrap();
        
        // 列出文件
        let files = backend.list("dir").await.unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.contains(&"file1.txt".to_string()));
    }

    #[tokio::test]
    async fn test_local_backend_delete() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalBackend::new(temp_dir.path().to_path_buf());
        
        let path = "test.txt";
        backend.write(path, b"data").await.unwrap();
        
        assert!(backend.exists(path).await.unwrap());
        
        backend.delete(path).await.unwrap();
        
        assert!(!backend.exists(path).await.unwrap());
    }

    #[tokio::test]
    async fn test_local_backend_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalBackend::new(temp_dir.path().to_path_buf());
        
        let path = "test.txt";
        let data = b"Hello, World!";
        
        backend.write(path, data).await.unwrap();
        
        let metadata = backend.metadata(path).await.unwrap();
        assert_eq!(metadata.size, data.len() as u64);
        assert!(metadata.modified_time > 0);
    }
}
