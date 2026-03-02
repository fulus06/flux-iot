use chrono::{DateTime, Duration, Utc};
use flux_storage::{LocalBackend, StorageBackend};
#[cfg(feature = "s3")]
use flux_storage::{S3Backend, S3Config};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// 归档目标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchiveDestination {
    /// S3 存储
    S3 {
        bucket: String,
        region: String,
        prefix: String,
    },
    /// MinIO 存储
    MinIO {
        endpoint: String,
        bucket: String,
        access_key: String,
        secret_key: String,
    },
    /// 本地文件
    LocalFile {
        path: String,
    },
}

/// 归档策略
#[derive(Debug, Clone)]
pub struct ArchivePolicy {
    /// 表名
    pub table_name: String,
    
    /// 归档时间阈值（早于此时间的数据将被归档）
    pub archive_older_than: Duration,
    
    /// 归档目标
    pub destination: ArchiveDestination,
    
    /// 是否在归档后删除原始数据
    pub delete_after_archive: bool,
}

impl ArchivePolicy {
    /// 生成归档文件名
    pub fn generate_filename(&self, base_path: &str) -> String {
        let now = Utc::now();
        let timestamp = now.format("%Y%m%d_%H%M%S");
        format!("{}/{}_{}.json", base_path, self.table_name, timestamp)
    }
    
    /// 生成按日期分组的归档文件名
    pub fn generate_daily_filename(&self, base_path: &str, date: DateTime<Utc>) -> String {
        let date_str = date.format("%Y-%m-%d");
        format!("{}/{}_{}.json", base_path, self.table_name, date_str)
    }
    
    /// 生成按月份分组的归档文件名
    pub fn generate_monthly_filename(&self, base_path: &str, date: DateTime<Utc>) -> String {
        let month_str = date.format("%Y-%m");
        format!("{}/{}_{}.json", base_path, self.table_name, month_str)
    }
    
    /// 生成按年份分组的归档文件名
    pub fn generate_yearly_filename(&self, base_path: &str, date: DateTime<Utc>) -> String {
        let year_str = date.format("%Y");
        format!("{}/{}_{}.json", base_path, self.table_name, year_str)
    }
}

/// 归档统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveStats {
    pub table_name: String,
    pub archived_rows: u64,
    pub archive_size_mb: f64,
    pub execution_time_ms: i64,
    pub archived_at: DateTime<Utc>,
    pub destination: String,
}

/// 数据归档器
pub struct DataArchiver {
    db: Arc<DatabaseConnection>,
    /// 存储后端（用于 S3/MinIO 归档）
    storage_backend: Option<Arc<dyn StorageBackend>>,
}

impl DataArchiver {
    /// 创建归档器（默认使用本地存储）
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self::new_with_path(db, PathBuf::from("/var/lib/flux-iot/archive"))
    }
    
    /// 创建归档器并指定本地存储路径
    pub fn new_with_path(db: Arc<DatabaseConnection>, storage_path: PathBuf) -> Self {
        let local_backend = LocalBackend::new(storage_path);
        Self { 
            db,
            storage_backend: Some(Arc::new(local_backend)),
        }
    }
    
    /// 创建带自定义存储后端的归档器（用于 S3/MinIO）
    pub fn with_storage(db: Arc<DatabaseConnection>, storage: Arc<dyn StorageBackend>) -> Self {
        Self {
            db,
            storage_backend: Some(storage),
        }
    }
    
    /// 从配置创建归档器（推荐）
    /// 
    /// 根据配置自动选择本地存储或 S3 存储
    /// 
    /// # 参数
    /// - `base_path`: 基础存储路径（来自 storage.base_path）
    /// - `s3_config`: S3 配置（来自 storage.s3）
    #[cfg(feature = "s3")]
    pub async fn from_config(
        db: Arc<DatabaseConnection>,
        base_path: String,
        s3_config: Option<S3Config>,
    ) -> anyhow::Result<Self> {
        if let Some(s3_cfg) = s3_config {
            let s3_backend = S3Backend::new(s3_cfg).await?;
            Ok(Self::with_storage(db, Arc::new(s3_backend)))
        } else {
            // 在基础路径下创建 archive 子目录
            let archive_path = PathBuf::from(base_path).join("archive");
            Ok(Self::new_with_path(db, archive_path))
        }
    }
    
    /// 从配置创建归档器（无 S3 支持）
    #[cfg(not(feature = "s3"))]
    pub async fn from_config(
        db: Arc<DatabaseConnection>,
        base_path: String,
        _s3_config: Option<()>,
    ) -> anyhow::Result<Self> {
        // 在基础路径下创建 archive 子目录
        let archive_path = PathBuf::from(base_path).join("archive");
        Ok(Self::new_with_path(db, archive_path))
    }

    /// 执行归档任务
    pub async fn archive(&self, policy: &ArchivePolicy) -> anyhow::Result<ArchiveStats> {
        let start_time = std::time::Instant::now();
        let cutoff_time = Utc::now() - policy.archive_older_than;

        info!(
            table = %policy.table_name,
            cutoff_time = %cutoff_time,
            "Starting archive task"
        );

        // 1. 查询需要归档的数据
        let data = self.query_old_data(&policy.table_name, cutoff_time).await?;
        let archived_rows = data.len() as u64;

        // 2. 导出数据
        let archive_size_mb = self.export_data(&data, &policy.destination, &policy.table_name).await?;

        // 3. 如果配置了删除，则删除原始数据
        if policy.delete_after_archive {
            self.delete_archived_data(&policy.table_name, cutoff_time).await?;
        }

        let execution_time_ms = start_time.elapsed().as_millis() as i64;

        let stats = ArchiveStats {
            table_name: policy.table_name.clone(),
            archived_rows,
            archive_size_mb,
            execution_time_ms,
            archived_at: Utc::now(),
            destination: format!("{:?}", policy.destination),
        };

        info!(
            table = %policy.table_name,
            archived_rows = %archived_rows,
            size_mb = %archive_size_mb,
            "Archive completed"
        );

        Ok(stats)
    }

    /// 查询旧数据
    async fn query_old_data(
        &self,
        table_name: &str,
        cutoff_time: DateTime<Utc>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let sql = format!(
            "SELECT * FROM {} WHERE time < $1 ORDER BY time",
            table_name
        );

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            vec![cutoff_time.into()],
        );

        let results = self.db.query_all(stmt).await?;

        let mut data = Vec::new();
        for row in results {
            // 将行转换为 JSON
            let json = serde_json::json!({
                "time": row.try_get::<DateTime<Utc>>("", "time").ok(),
                "device_id": row.try_get::<String>("", "device_id").ok(),
                "metric_name": row.try_get::<String>("", "metric_name").ok(),
                "metric_value": row.try_get::<f64>("", "metric_value").ok(),
            });
            data.push(json);
        }

        debug!(count = data.len(), "Old data queried");
        Ok(data)
    }

    /// 导出数据到目标
    async fn export_data(
        &self,
        data: &[serde_json::Value],
        destination: &ArchiveDestination,
        table_name: &str,
    ) -> anyhow::Result<f64> {
        match destination {
            ArchiveDestination::LocalFile { path } => {
                // 如果 path 是目录，则生成动态文件名
                let final_path = if path.ends_with('/') || std::path::Path::new(path).is_dir() {
                    let now = Utc::now();
                    let timestamp = now.format("%Y%m%d_%H%M%S");
                    format!("{}/{}_{}.json", path.trim_end_matches('/'), table_name, timestamp)
                } else {
                    path.clone()
                };
                
                self.export_to_local_file(data, &final_path).await
            }
            ArchiveDestination::S3 { bucket: _, region: _, prefix } => {
                if let Some(backend) = &self.storage_backend {
                    let now = Utc::now();
                    let timestamp = now.format("%Y%m%d_%H%M%S");
                    let object_key = format!("{}/{}_{}.json", prefix, table_name, timestamp);
                    
                    self.export_to_storage(data, backend.as_ref(), &object_key).await
                } else {
                    warn!("Storage backend not configured for S3 export");
                    Err(anyhow::anyhow!("Storage backend not configured"))
                }
            }
            ArchiveDestination::MinIO { endpoint: _, bucket: _, access_key: _, secret_key: _ } => {
                if let Some(backend) = &self.storage_backend {
                    let now = Utc::now();
                    let timestamp = now.format("%Y%m%d_%H%M%S");
                    let object_key = format!("{}_{}.json", table_name, timestamp);
                    
                    self.export_to_storage(data, backend.as_ref(), &object_key).await
                } else {
                    warn!("Storage backend not configured for MinIO export");
                    Err(anyhow::anyhow!("Storage backend not configured"))
                }
            }
        }
    }

    /// 导出到本地文件
    async fn export_to_local_file(
        &self,
        data: &[serde_json::Value],
        path: &str,
    ) -> anyhow::Result<f64> {
        use tokio::fs::{create_dir_all, File};
        use tokio::io::AsyncWriteExt;

        // 确保目录存在
        if let Some(parent) = std::path::Path::new(path).parent() {
            create_dir_all(parent).await?;
        }

        let json_data = serde_json::to_string_pretty(data)?;
        let size_mb = json_data.len() as f64 / 1024.0 / 1024.0;

        let mut file = File::create(path).await?;
        file.write_all(json_data.as_bytes()).await?;

        info!(path = %path, size_mb = %size_mb, "Data exported to local file");
        Ok(size_mb)
    }

    /// 导出到存储后端（S3/MinIO）
    async fn export_to_storage(
        &self,
        data: &[serde_json::Value],
        backend: &dyn StorageBackend,
        object_key: &str,
    ) -> anyhow::Result<f64> {
        let json_data = serde_json::to_string_pretty(data)?;
        let size_mb = json_data.len() as f64 / 1024.0 / 1024.0;
        
        backend.write(object_key, json_data.as_bytes()).await?;
        
        info!(
            key = %object_key,
            size_mb = %size_mb,
            rows = data.len(),
            "Data exported to storage backend"
        );
        
        Ok(size_mb)
    }

    /// 删除已归档的数据
    async fn delete_archived_data(
        &self,
        table_name: &str,
        cutoff_time: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            "DELETE FROM {} WHERE time < $1",
            table_name
        );

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            vec![cutoff_time.into()],
        );

        let result = self.db.execute(stmt).await?;
        
        debug!(
            table = %table_name,
            deleted_rows = %result.rows_affected(),
            "Archived data deleted"
        );

        Ok(())
    }

    /// 恢复归档数据（从本地文件）
    pub async fn restore_from_file(&self, file_path: &str, table_name: &str) -> anyhow::Result<u64> {
        use tokio::fs::File;
        use tokio::io::AsyncReadExt;

        let mut file = File::open(file_path).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;

        let data: Vec<serde_json::Value> = serde_json::from_str(&contents)?;
        
        self.restore_data(table_name, &data).await
    }
    
    /// 恢复归档数据（从存储后端）
    pub async fn restore_from_storage(&self, object_key: &str, table_name: &str) -> anyhow::Result<u64> {
        if let Some(backend) = &self.storage_backend {
            let data_bytes = backend.read(object_key).await?;
            let contents = String::from_utf8(data_bytes.to_vec())?;
            let data: Vec<serde_json::Value> = serde_json::from_str(&contents)?;
            
            self.restore_data(table_name, &data).await
        } else {
            Err(anyhow::anyhow!("Storage backend not configured"))
        }
    }
    
    /// 恢复数据到数据库
    async fn restore_data(&self, table_name: &str, data: &[serde_json::Value]) -> anyhow::Result<u64> {
        let mut restored_count = 0u64;
        
        for record in data {
            // 构造 INSERT 语句
            let time = record.get("time")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<DateTime<Utc>>().ok())
                .unwrap_or_else(Utc::now);
            
            let device_id = record.get("device_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            let metric_name = record.get("metric_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            let metric_value = record.get("metric_value")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            
            let sql = format!(
                "INSERT INTO {} (time, device_id, metric_name, metric_value) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
                table_name
            );
            
            let stmt = Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                sql,
                vec![
                    time.into(),
                    device_id.into(),
                    metric_name.into(),
                    metric_value.into(),
                ],
            );
            
            match self.db.execute(stmt).await {
                Ok(result) => {
                    restored_count += result.rows_affected();
                }
                Err(e) => {
                    warn!(error = %e, "Failed to restore record");
                }
            }
        }
        
        info!(
            table = %table_name,
            restored_rows = restored_count,
            "Archive data restored"
        );
        
        Ok(restored_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_destination() {
        let dest = ArchiveDestination::LocalFile {
            path: "/tmp/archive".to_string(),
        };

        match dest {
            ArchiveDestination::LocalFile { path } => {
                assert_eq!(path, "/tmp/archive");
            }
            _ => panic!("Wrong destination type"),
        }
    }
}
