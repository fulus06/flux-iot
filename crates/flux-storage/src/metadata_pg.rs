#[cfg(feature = "postgres")]
use anyhow::Result;
#[cfg(feature = "postgres")]
use serde_json::Value as JsonValue;
#[cfg(feature = "postgres")]
use sqlx::{PgPool, Row};
#[cfg(feature = "postgres")]
use std::collections::HashMap;
#[cfg(feature = "postgres")]
use tracing::{debug, info};

#[cfg(feature = "postgres")]
use crate::segment::SegmentMetadata;

/// PostgreSQL 元数据后端
#[cfg(feature = "postgres")]
pub struct PostgresMetadataBackend {
    pool: PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresMetadataBackend {
    /// 创建新的 PostgreSQL 元数据后端
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 从数据库 URL 创建
    pub async fn from_url(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    /// 运行迁移
    pub async fn run_migrations(&self) -> Result<()> {
        // 读取迁移 SQL
        let migration_sql = include_str!("../migrations/001_create_storage_schema.sql");
        
        sqlx::query(migration_sql)
            .execute(&self.pool)
            .await?;
        
        info!("PostgreSQL migrations completed");
        Ok(())
    }

    /// 保存元数据
    pub async fn save_metadata(
        &self,
        stream_id: &str,
        segment_id: u64,
        metadata: &SegmentMetadata,
    ) -> Result<()> {
        // 将 HashMap 转换为 JSONB
        let metadata_json = serde_json::to_value(&metadata.metadata)?;

        sqlx::query(
            r#"
            INSERT INTO storage.segment_metadata (stream_id, segment_id, metadata)
            VALUES ($1, $2, $3)
            ON CONFLICT (stream_id, segment_id)
            DO UPDATE SET metadata = $3, updated_at = NOW()
            "#,
        )
        .bind(stream_id)
        .bind(segment_id as i64)
        .bind(metadata_json)
        .execute(&self.pool)
        .await?;

        debug!(
            stream_id = %stream_id,
            segment_id = segment_id,
            "Metadata saved to PostgreSQL"
        );

        Ok(())
    }

    /// 获取元数据
    pub async fn get_metadata(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<SegmentMetadata> {
        let row = sqlx::query(
            r#"
            SELECT metadata
            FROM storage.segment_metadata
            WHERE stream_id = $1 AND segment_id = $2
            "#,
        )
        .bind(stream_id)
        .bind(segment_id as i64)
        .fetch_one(&self.pool)
        .await?;

        let metadata_json: JsonValue = row.try_get("metadata")?;
        let metadata_map: HashMap<String, String> = serde_json::from_value(metadata_json)?;

        Ok(SegmentMetadata {
            metadata: metadata_map,
        })
    }

    /// 查询元数据
    /// 
    /// 使用 PostgreSQL JSONB 查询能力
    pub async fn query_metadata(
        &self,
        stream_id: &str,
        filter: HashMap<String, String>,
    ) -> Result<Vec<(u64, SegmentMetadata)>> {
        let mut query = String::from(
            "SELECT segment_id, metadata FROM storage.segment_metadata WHERE stream_id = $1"
        );

        // 构建 JSONB 查询条件
        let mut param_index = 2;
        let mut params: Vec<String> = vec![stream_id.to_string()];

        for (key, value) in &filter {
            query.push_str(&format!(
                " AND metadata->>'{}' = ${}",
                key, param_index
            ));
            params.push(value.clone());
            param_index += 1;
        }

        query.push_str(" ORDER BY segment_id");

        // 执行查询
        let mut sql_query = sqlx::query(&query);
        for param in &params {
            sql_query = sql_query.bind(param);
        }

        let rows = sql_query.fetch_all(&self.pool).await?;

        let mut results = Vec::new();
        for row in rows {
            let segment_id: i64 = row.try_get("segment_id")?;
            let metadata_json: JsonValue = row.try_get("metadata")?;
            let metadata_map: HashMap<String, String> = serde_json::from_value(metadata_json)?;

            results.push((
                segment_id as u64,
                SegmentMetadata {
                    metadata: metadata_map,
                },
            ));
        }

        debug!(
            stream_id = %stream_id,
            count = results.len(),
            "Metadata queried from PostgreSQL"
        );

        Ok(results)
    }

    /// 删除元数据
    pub async fn delete_metadata(&self, stream_id: &str, segment_id: u64) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM storage.segment_metadata
            WHERE stream_id = $1 AND segment_id = $2
            "#,
        )
        .bind(stream_id)
        .bind(segment_id as i64)
        .execute(&self.pool)
        .await?;

        debug!(
            stream_id = %stream_id,
            segment_id = segment_id,
            "Metadata deleted from PostgreSQL"
        );

        Ok(())
    }

    /// 清理过期元数据
    pub async fn cleanup_old_metadata(
        &self,
        stream_id: &str,
        keep_count: usize,
    ) -> Result<usize> {
        let result = sqlx::query(
            r#"
            DELETE FROM storage.segment_metadata
            WHERE stream_id = $1
            AND segment_id NOT IN (
                SELECT segment_id
                FROM storage.segment_metadata
                WHERE stream_id = $1
                ORDER BY segment_id DESC
                LIMIT $2
            )
            "#,
        )
        .bind(stream_id)
        .bind(keep_count as i64)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected() as usize;

        info!(
            stream_id = %stream_id,
            deleted = deleted,
            kept = keep_count,
            "Old metadata cleaned up from PostgreSQL"
        );

        Ok(deleted)
    }

    /// 获取连接池
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[cfg(test)]
#[cfg(feature = "postgres")]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要 PostgreSQL 数据库
    async fn test_postgres_metadata_backend() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/flux_iot_test".to_string());

        let backend = PostgresMetadataBackend::from_url(&database_url)
            .await
            .unwrap();

        backend.run_migrations().await.unwrap();

        // 保存元数据
        let mut metadata = SegmentMetadata::new();
        metadata
            .set("start_time", "2026-02-23T15:00:00Z")
            .set("duration", "10.0")
            .set("has_keyframe", "true");

        backend
            .save_metadata("test/stream", 1, &metadata)
            .await
            .unwrap();

        // 获取元数据
        let loaded = backend.get_metadata("test/stream", 1).await.unwrap();
        assert_eq!(loaded.get("duration"), Some(&"10.0".to_string()));

        // 查询元数据
        let mut filter = HashMap::new();
        filter.insert("has_keyframe".to_string(), "true".to_string());

        let results = backend
            .query_metadata("test/stream", filter)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // 删除元数据
        backend.delete_metadata("test/stream", 1).await.unwrap();
    }
}
