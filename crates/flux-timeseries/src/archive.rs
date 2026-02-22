use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

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
}

impl DataArchiver {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
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
            ArchiveDestination::S3 { bucket, region, prefix } => {
                // 生成 S3 对象键
                let now = Utc::now();
                let timestamp = now.format("%Y%m%d_%H%M%S");
                let object_key = format!("{}/{}_{}.json", prefix, table_name, timestamp);
                
                info!(
                    bucket = %bucket,
                    region = %region,
                    key = %object_key,
                    "S3 export not implemented yet"
                );
                Ok(0.0)
            }
            ArchiveDestination::MinIO { endpoint, bucket, .. } => {
                // 生成 MinIO 对象键
                let now = Utc::now();
                let timestamp = now.format("%Y%m%d_%H%M%S");
                let object_key = format!("{}_{}.json", table_name, timestamp);
                
                info!(
                    endpoint = %endpoint,
                    bucket = %bucket,
                    key = %object_key,
                    "MinIO export not implemented yet"
                );
                Ok(0.0)
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

    /// 恢复归档数据
    pub async fn restore_from_file(&self, file_path: &str, table_name: &str) -> anyhow::Result<u64> {
        use tokio::fs::File;
        use tokio::io::AsyncReadExt;

        let mut file = File::open(file_path).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;

        let data: Vec<serde_json::Value> = serde_json::from_str(&contents)?;
        
        // TODO: 实现数据恢复逻辑
        info!(
            file = %file_path,
            table = %table_name,
            rows = data.len(),
            "Archive restore not fully implemented yet"
        );

        Ok(data.len() as u64)
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
