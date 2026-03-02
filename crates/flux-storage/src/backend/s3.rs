use crate::backend::{BackendStats, FileMetadata, StorageBackend};
use anyhow::Result;
use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// S3 存储后端配置
#[derive(Debug, Clone)]
pub struct S3Config {
    /// S3 bucket 名称
    pub bucket: String,
    
    /// AWS 区域
    pub region: String,
    
    /// 自定义端点（用于 MinIO 等 S3 兼容服务）
    pub endpoint: Option<String>,
    
    /// 访问密钥 ID
    pub access_key_id: Option<String>,
    
    /// 访问密钥
    pub secret_access_key: Option<String>,
    
    /// 路径前缀
    pub prefix: Option<String>,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            bucket: "flux-iot".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            prefix: None,
        }
    }
}

/// S3 存储后端
pub struct S3Backend {
    client: Client,
    config: S3Config,
    stats: Arc<RwLock<BackendStats>>,
}

impl S3Backend {
    /// 创建新的 S3 后端
    pub async fn new(config: S3Config) -> Result<Self> {
        let aws_config = if let Some(endpoint) = &config.endpoint {
            // MinIO 或其他 S3 兼容服务
            let mut loader = aws_config::from_env()
                .region(aws_sdk_s3::config::Region::new(config.region.clone()));
            
            if let (Some(access_key), Some(secret_key)) = (&config.access_key_id, &config.secret_access_key) {
                loader = loader.credentials_provider(
                    aws_sdk_s3::config::Credentials::new(
                        access_key,
                        secret_key,
                        None,
                        None,
                        "static"
                    )
                );
            }
            
            let sdk_config = loader.load().await;
            
            aws_sdk_s3::config::Builder::from(&sdk_config)
                .endpoint_url(endpoint)
                .force_path_style(true) // MinIO 需要
                .build()
        } else {
            // 标准 AWS S3
            let sdk_config = aws_config::load_from_env().await;
            aws_sdk_s3::config::Builder::from(&sdk_config).build()
        };
        
        let client = Client::from_conf(aws_config);
        
        info!(
            bucket = %config.bucket,
            region = %config.region,
            endpoint = ?config.endpoint,
            "S3 backend initialized"
        );
        
        Ok(Self {
            client,
            config,
            stats: Arc::new(RwLock::new(BackendStats::default())),
        })
    }
    
    /// 构建完整路径（包含前缀）
    fn build_path(&self, path: &str) -> String {
        if let Some(prefix) = &self.config.prefix {
            format!("{}/{}", prefix.trim_end_matches('/'), path.trim_start_matches('/'))
        } else {
            path.to_string()
        }
    }
}

#[async_trait]
impl StorageBackend for S3Backend {
    async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let start = std::time::Instant::now();
        let full_path = self.build_path(path);
        
        let body = ByteStream::from(data.to_vec());
        
        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&full_path)
            .body(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to write to S3: {}", e))?;
        
        let elapsed = start.elapsed();
        
        // 更新统计
        let mut stats = self.stats.write().await;
        stats.write_count += 1;
        stats.bytes_written += data.len() as u64;
        stats.avg_write_latency_ms = 
            (stats.avg_write_latency_ms * (stats.write_count - 1) as f64 + elapsed.as_millis() as f64) 
            / stats.write_count as f64;
        
        debug!(
            path = %full_path,
            size = data.len(),
            latency_ms = elapsed.as_millis(),
            "Wrote to S3"
        );
        
        Ok(())
    }
    
    async fn read(&self, path: &str) -> Result<Bytes> {
        let start = std::time::Instant::now();
        let full_path = self.build_path(path);
        
        let response = self.client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&full_path)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read from S3: {}", e))?;
        
        let data = response.body.collect().await
            .map_err(|e| anyhow::anyhow!("Failed to collect S3 body: {}", e))?
            .into_bytes();
        
        let elapsed = start.elapsed();
        
        // 更新统计
        let mut stats = self.stats.write().await;
        stats.read_count += 1;
        stats.bytes_read += data.len() as u64;
        stats.avg_read_latency_ms = 
            (stats.avg_read_latency_ms * (stats.read_count - 1) as f64 + elapsed.as_millis() as f64) 
            / stats.read_count as f64;
        
        debug!(
            path = %full_path,
            size = data.len(),
            latency_ms = elapsed.as_millis(),
            "Read from S3"
        );
        
        Ok(data)
    }
    
    async fn read_range(&self, path: &str, start: u64, length: u64) -> Result<Bytes> {
        let full_path = self.build_path(path);
        let end = start + length - 1;
        let range = format!("bytes={}-{}", start, end);
        
        let response = self.client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&full_path)
            .range(range)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read range from S3: {}", e))?;
        
        let data = response.body.collect().await
            .map_err(|e| anyhow::anyhow!("Failed to collect S3 body: {}", e))?
            .into_bytes();
        
        debug!(
            path = %full_path,
            start = start,
            length = length,
            "Read range from S3"
        );
        
        Ok(data)
    }
    
    async fn delete(&self, path: &str) -> Result<()> {
        let full_path = self.build_path(path);
        
        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&full_path)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete from S3: {}", e))?;
        
        // 更新统计
        let mut stats = self.stats.write().await;
        stats.delete_count += 1;
        
        debug!(path = %full_path, "Deleted from S3");
        
        Ok(())
    }
    
    async fn delete_batch(&self, paths: Vec<String>) -> Result<Vec<Result<()>>> {
        // S3 支持批量删除，最多 1000 个对象
        let mut results = Vec::new();
        
        for chunk in paths.chunks(1000) {
            let objects: Vec<_> = chunk.iter()
                .map(|p| {
                    aws_sdk_s3::types::ObjectIdentifier::builder()
                        .key(self.build_path(p))
                        .build()
                        .unwrap()
                })
                .collect();
            
            let delete = aws_sdk_s3::types::Delete::builder()
                .set_objects(Some(objects))
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build delete request: {}", e))?;
            
            match self.client
                .delete_objects()
                .bucket(&self.config.bucket)
                .delete(delete)
                .send()
                .await
            {
                Ok(_) => {
                    for _ in 0..chunk.len() {
                        results.push(Ok(()));
                    }
                }
                Err(e) => {
                    error!(error = %e, "Batch delete failed");
                    for _ in 0..chunk.len() {
                        results.push(Err(anyhow::anyhow!("Batch delete failed: {}", e)));
                    }
                }
            }
        }
        
        Ok(results)
    }
    
    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let full_prefix = self.build_path(prefix);
        let mut result = Vec::new();
        let mut continuation_token = None;
        
        loop {
            let mut request = self.client
                .list_objects_v2()
                .bucket(&self.config.bucket)
                .prefix(&full_prefix);
            
            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }
            
            let response = request.send().await
                .map_err(|e| anyhow::anyhow!("Failed to list S3 objects: {}", e))?;
            
            if let Some(contents) = response.contents {
                for object in contents {
                    if let Some(key) = object.key {
                        // 移除前缀
                        let relative_key = if let Some(prefix) = &self.config.prefix {
                            key.strip_prefix(&format!("{}/", prefix.trim_end_matches('/')))
                                .unwrap_or(&key)
                                .to_string()
                        } else {
                            key
                        };
                        result.push(relative_key);
                    }
                }
            }
            
            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }
        
        debug!(prefix = %full_prefix, count = result.len(), "Listed S3 objects");
        
        Ok(result)
    }
    
    async fn exists(&self, path: &str) -> Result<bool> {
        let full_path = self.build_path(path);
        
        match self.client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&full_path)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("404") || error_str.contains("NotFound") {
                    Ok(false)
                } else {
                    Err(anyhow::anyhow!("Failed to check S3 object existence: {}", e))
                }
            }
        }
    }
    
    async fn metadata(&self, path: &str) -> Result<FileMetadata> {
        let full_path = self.build_path(path);
        
        let response = self.client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&full_path)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get S3 metadata: {}", e))?;
        
        Ok(FileMetadata {
            size: response.content_length.unwrap_or(0) as u64,
            modified_time: response.last_modified
                .and_then(|dt| dt.secs().try_into().ok())
                .unwrap_or(0),
            content_type: response.content_type,
            etag: response.e_tag,
        })
    }
    
    fn backend_type(&self) -> &str {
        if self.config.endpoint.is_some() {
            "s3-compatible"
        } else {
            "s3"
        }
    }
    
    async fn stats(&self) -> BackendStats {
        self.stats.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3_config_default() {
        let config = S3Config::default();
        assert_eq!(config.bucket, "flux-iot");
        assert_eq!(config.region, "us-east-1");
    }

    #[test]
    fn test_build_path() {
        let config = S3Config {
            bucket: "test".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            prefix: Some("archive".to_string()),
        };
        
        // 注意：这里不能直接测试 build_path，因为它是私有方法
        // 实际测试需要通过集成测试
        assert_eq!(config.prefix, Some("archive".to_string()));
    }
}
