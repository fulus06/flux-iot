use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// 清理策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPolicy {
    /// 表名
    pub table_name: String,
    
    /// 数据保留时间
    pub retention: Duration,
    
    /// 是否启用
    pub enabled: bool,
}

impl CleanupPolicy {
    pub fn for_metrics() -> Self {
        Self {
            table_name: "device_metrics".to_string(),
            retention: Duration::days(90),
            enabled: true,
        }
    }

    pub fn for_logs() -> Self {
        Self {
            table_name: "device_logs".to_string(),
            retention: Duration::days(30),
            enabled: true,
        }
    }

    pub fn for_events() -> Self {
        Self {
            table_name: "device_events".to_string(),
            retention: Duration::days(180),
            enabled: true,
        }
    }
}

/// 清理统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupStats {
    pub table_name: String,
    pub deleted_rows: u64,
    pub freed_space_mb: f64,
    pub execution_time_ms: i64,
    pub executed_at: DateTime<Utc>,
}

/// 存储统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_size_mb: f64,
    pub compressed_size_mb: f64,
    pub compression_ratio: f64,
    pub table_sizes: HashMap<String, f64>,
    pub chunk_count: usize,
}

/// 清理管理器
pub struct CleanupManager {
    db: Arc<DatabaseConnection>,
}

impl CleanupManager {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 执行清理任务
    pub async fn cleanup(&self, policy: &CleanupPolicy) -> anyhow::Result<CleanupStats> {
        if !policy.enabled {
            warn!(table = %policy.table_name, "Cleanup policy is disabled");
            return Ok(CleanupStats {
                table_name: policy.table_name.clone(),
                deleted_rows: 0,
                freed_space_mb: 0.0,
                execution_time_ms: 0,
                executed_at: Utc::now(),
            });
        }

        let start_time = std::time::Instant::now();
        let cutoff_time = Utc::now() - policy.retention;

        // 获取清理前的大小
        let size_before = self.get_table_size(&policy.table_name).await?;

        // 删除过期数据
        let sql = format!(
            "DELETE FROM {} WHERE time < $1",
            policy.table_name
        );

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            vec![cutoff_time.into()],
        );

        let result = self.db.execute(stmt).await?;
        let deleted_rows = result.rows_affected();

        // 执行 VACUUM 释放空间
        self.vacuum_table(&policy.table_name).await?;

        // 获取清理后的大小
        let size_after = self.get_table_size(&policy.table_name).await?;
        let freed_space_mb = size_before - size_after;

        let execution_time_ms = start_time.elapsed().as_millis() as i64;

        let stats = CleanupStats {
            table_name: policy.table_name.clone(),
            deleted_rows,
            freed_space_mb,
            execution_time_ms,
            executed_at: Utc::now(),
        };

        info!(
            table = %policy.table_name,
            deleted_rows = %deleted_rows,
            freed_mb = %freed_space_mb,
            "Cleanup completed"
        );

        Ok(stats)
    }

    /// 获取表大小（MB）
    async fn get_table_size(&self, table_name: &str) -> anyhow::Result<f64> {
        let sql = format!(
            "SELECT pg_total_relation_size('{}') as size",
            table_name
        );

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        let result = self.db.query_one(stmt).await?;

        if let Some(row) = result {
            let size_bytes: i64 = row.try_get("", "size")?;
            Ok(size_bytes as f64 / 1024.0 / 1024.0)
        } else {
            Ok(0.0)
        }
    }

    /// 执行 VACUUM
    async fn vacuum_table(&self, table_name: &str) -> anyhow::Result<()> {
        let sql = format!("VACUUM ANALYZE {}", table_name);
        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        self.db.execute(stmt).await?;

        debug!(table = %table_name, "Table vacuumed");
        Ok(())
    }

    /// 获取存储统计
    pub async fn get_storage_stats(&self) -> anyhow::Result<StorageStats> {
        // 获取所有表的大小
        let mut table_sizes = HashMap::new();
        let tables = vec!["device_metrics", "device_logs", "device_events"];

        let mut total_size_mb = 0.0;
        for table in &tables {
            let size = self.get_table_size(table).await?;
            table_sizes.insert(table.to_string(), size);
            total_size_mb += size;
        }

        // 获取压缩统计
        let compression_stats = self.get_compression_stats().await?;
        let compressed_size_mb = compression_stats.0;
        let compression_ratio = if compressed_size_mb > 0.0 {
            total_size_mb / compressed_size_mb
        } else {
            1.0
        };

        // 获取 Chunk 数量
        let chunk_count = self.get_chunk_count().await?;

        Ok(StorageStats {
            total_size_mb,
            compressed_size_mb,
            compression_ratio,
            table_sizes,
            chunk_count,
        })
    }

    /// 获取压缩统计
    async fn get_compression_stats(&self) -> anyhow::Result<(f64, f64)> {
        let sql = r#"
            SELECT 
                SUM(before_compression_total_bytes) / 1024.0 / 1024.0 as uncompressed_mb,
                SUM(after_compression_total_bytes) / 1024.0 / 1024.0 as compressed_mb
            FROM timescaledb_information.compression_settings
        "#;

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql.to_string());
        let result = self.db.query_one(stmt).await?;

        if let Some(row) = result {
            let compressed_mb: Option<f64> = row.try_get("", "compressed_mb").ok();
            let uncompressed_mb: Option<f64> = row.try_get("", "uncompressed_mb").ok();
            Ok((compressed_mb.unwrap_or(0.0), uncompressed_mb.unwrap_or(0.0)))
        } else {
            Ok((0.0, 0.0))
        }
    }

    /// 获取 Chunk 数量
    async fn get_chunk_count(&self) -> anyhow::Result<usize> {
        let sql = r#"
            SELECT COUNT(*) as count
            FROM timescaledb_information.chunks
        "#;

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql.to_string());
        let result = self.db.query_one(stmt).await?;

        if let Some(row) = result {
            let count: i64 = row.try_get("", "count")?;
            Ok(count as usize)
        } else {
            Ok(0)
        }
    }

    /// 手动压缩指定表
    pub async fn compress_table(&self, table_name: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            SELECT compress_chunk(i) 
            FROM show_chunks('{}') i
            "#,
            table_name
        );

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        self.db.execute(stmt).await?;

        info!(table = %table_name, "Table compressed manually");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_policy() {
        let policy = CleanupPolicy::for_metrics();
        assert_eq!(policy.table_name, "device_metrics");
        assert_eq!(policy.retention.num_days(), 90);
        assert!(policy.enabled);
    }

    #[test]
    fn test_logs_policy() {
        let policy = CleanupPolicy::for_logs();
        assert_eq!(policy.retention.num_days(), 30);
    }
}
