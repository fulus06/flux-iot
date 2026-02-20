use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;

pub mod local;

pub use local::LocalBackend;

/// 存储后端抽象 trait
/// 
/// 提供统一的存储接口，支持本地文件系统、S3、OSS 等多种后端
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 写入文件
    /// 
    /// # 性能优化
    /// - 使用零拷贝技术
    /// - 异步 I/O
    /// - 批量写入支持
    async fn write(&self, path: &str, data: &[u8]) -> Result<()>;
    
    /// 批量写入文件（性能优化）
    /// 
    /// 默认实现为逐个写入，具体后端可以优化为真正的批量操作
    async fn write_batch(&self, items: Vec<(String, Vec<u8>)>) -> Result<Vec<Result<()>>> {
        let mut results = Vec::with_capacity(items.len());
        for (path, data) in items {
            results.push(self.write(&path, &data).await);
        }
        Ok(results)
    }
    
    /// 读取文件
    /// 
    /// # 性能优化
    /// - 返回 Bytes（零拷贝）
    /// - 支持范围读取
    async fn read(&self, path: &str) -> Result<Bytes>;
    
    /// 范围读取（性能优化）
    /// 
    /// 读取文件的指定范围，避免读取整个文件
    async fn read_range(&self, path: &str, start: u64, length: u64) -> Result<Bytes> {
        // 默认实现：读取全部然后切片（子类可以优化）
        let data = self.read(path).await?;
        let end = (start + length).min(data.len() as u64) as usize;
        Ok(data.slice(start as usize..end))
    }
    
    /// 删除文件
    async fn delete(&self, path: &str) -> Result<()>;
    
    /// 批量删除（性能优化）
    async fn delete_batch(&self, paths: Vec<String>) -> Result<Vec<Result<()>>> {
        let mut results = Vec::with_capacity(paths.len());
        for path in paths {
            results.push(self.delete(&path).await);
        }
        Ok(results)
    }
    
    /// 列出文件
    /// 
    /// # 性能优化
    /// - 支持分页
    /// - 支持前缀过滤
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
    
    /// 分页列出文件（性能优化）
    async fn list_page(
        &self,
        prefix: &str,
        page_size: usize,
        continuation_token: Option<String>,
    ) -> Result<(Vec<String>, Option<String>)> {
        // 默认实现：一次性列出所有（子类可以优化为真正的分页）
        let all = self.list(prefix).await?;
        let start = continuation_token
            .and_then(|t| t.parse::<usize>().ok())
            .unwrap_or(0);
        
        let end = (start + page_size).min(all.len());
        let items = all[start..end].to_vec();
        let next_token = if end < all.len() {
            Some(end.to_string())
        } else {
            None
        };
        
        Ok((items, next_token))
    }
    
    /// 检查文件是否存在
    async fn exists(&self, path: &str) -> Result<bool>;
    
    /// 获取文件元数据（性能优化）
    async fn metadata(&self, path: &str) -> Result<FileMetadata>;
    
    /// 获取后端类型
    fn backend_type(&self) -> &str;
    
    /// 获取后端统计信息（性能监控）
    async fn stats(&self) -> BackendStats {
        BackendStats::default()
    }
}

/// 文件元数据
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// 文件大小（字节）
    pub size: u64,
    
    /// 最后修改时间（Unix 时间戳）
    pub modified_time: u64,
    
    /// 内容类型
    pub content_type: Option<String>,
    
    /// ETag（用于缓存验证）
    pub etag: Option<String>,
}

/// 后端统计信息
#[derive(Debug, Clone, Default)]
pub struct BackendStats {
    /// 总读取次数
    pub read_count: u64,
    
    /// 总写入次数
    pub write_count: u64,
    
    /// 总删除次数
    pub delete_count: u64,
    
    /// 总读取字节数
    pub bytes_read: u64,
    
    /// 总写入字节数
    pub bytes_written: u64,
    
    /// 平均读取延迟（毫秒）
    pub avg_read_latency_ms: f64,
    
    /// 平均写入延迟（毫秒）
    pub avg_write_latency_ms: f64,
    
    /// 缓存命中率
    pub cache_hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_metadata() {
        let metadata = FileMetadata {
            size: 1024,
            modified_time: 1234567890,
            content_type: Some("video/mp2t".to_string()),
            etag: Some("abc123".to_string()),
        };
        
        assert_eq!(metadata.size, 1024);
    }

    #[test]
    fn test_backend_stats_default() {
        let stats = BackendStats::default();
        assert_eq!(stats.read_count, 0);
        assert_eq!(stats.write_count, 0);
    }
}
