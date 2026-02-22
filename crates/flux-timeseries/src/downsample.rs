use chrono::Duration;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use std::sync::Arc;
use tracing::{debug, info};

/// 降采样策略
#[derive(Debug, Clone)]
pub struct DownsamplePolicy {
    /// 源视图名称
    pub source_view: String,
    
    /// 目标视图名称
    pub target_view: String,
    
    /// 时间桶大小（秒）
    pub time_bucket_seconds: i64,
    
    /// 数据保留时间
    pub retention: Duration,
    
    /// 刷新间隔（秒）
    pub refresh_interval_seconds: i64,
}

impl DownsamplePolicy {
    pub fn daily() -> Self {
        Self {
            source_view: "device_metrics".to_string(),
            target_view: "device_metrics_1d".to_string(),
            time_bucket_seconds: 86400, // 1 day
            retention: Duration::days(365),
            refresh_interval_seconds: 3600, // 1 hour
        }
    }

    pub fn weekly() -> Self {
        Self {
            source_view: "device_metrics".to_string(),
            target_view: "device_metrics_1w".to_string(),
            time_bucket_seconds: 604800, // 1 week
            retention: Duration::days(730), // 2 years
            refresh_interval_seconds: 86400, // 1 day
        }
    }

    pub fn monthly() -> Self {
        Self {
            source_view: "device_metrics".to_string(),
            target_view: "device_metrics_1m".to_string(),
            time_bucket_seconds: 2592000, // 30 days
            retention: Duration::days(1825), // 5 years
            refresh_interval_seconds: 86400, // 1 day
        }
    }
}

/// 降采样管理器
pub struct DownsampleManager {
    db: Arc<DatabaseConnection>,
}

impl DownsampleManager {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 创建降采样视图
    pub async fn create_downsample_view(
        &self,
        policy: &DownsamplePolicy,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            CREATE MATERIALIZED VIEW IF NOT EXISTS {}
            WITH (timescaledb.continuous) AS
            SELECT 
                time_bucket('{} seconds', time) AS bucket,
                device_id,
                metric_name,
                AVG(metric_value) as avg_value,
                MAX(metric_value) as max_value,
                MIN(metric_value) as min_value,
                COUNT(*) as count,
                STDDEV(metric_value) as stddev_value
            FROM {}
            GROUP BY bucket, device_id, metric_name
            WITH NO DATA
            "#,
            policy.target_view,
            policy.time_bucket_seconds,
            policy.source_view
        );

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        self.db.execute(stmt).await?;

        info!(
            target_view = %policy.target_view,
            time_bucket = %policy.time_bucket_seconds,
            "Downsample view created"
        );

        Ok(())
    }

    /// 添加连续聚合刷新策略
    pub async fn add_refresh_policy(
        &self,
        policy: &DownsamplePolicy,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            SELECT add_continuous_aggregate_policy('{}',
                start_offset => INTERVAL '1 day',
                end_offset => INTERVAL '{} seconds',
                schedule_interval => INTERVAL '{} seconds',
                if_not_exists => TRUE
            )
            "#,
            policy.target_view,
            policy.time_bucket_seconds,
            policy.refresh_interval_seconds
        );

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        self.db.execute(stmt).await?;

        info!(
            target_view = %policy.target_view,
            refresh_interval = %policy.refresh_interval_seconds,
            "Refresh policy added"
        );

        Ok(())
    }

    /// 添加数据保留策略
    pub async fn add_retention_policy(
        &self,
        policy: &DownsamplePolicy,
    ) -> anyhow::Result<()> {
        let retention_days = policy.retention.num_days();
        
        let sql = format!(
            r#"
            SELECT add_retention_policy('{}',
                INTERVAL '{} days',
                if_not_exists => TRUE
            )
            "#,
            policy.target_view,
            retention_days
        );

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        self.db.execute(stmt).await?;

        info!(
            target_view = %policy.target_view,
            retention_days = %retention_days,
            "Retention policy added"
        );

        Ok(())
    }

    /// 手动刷新降采样视图
    pub async fn refresh_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!(
            "CALL refresh_continuous_aggregate('{}', NULL, NULL)",
            view_name
        );

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        self.db.execute(stmt).await?;

        debug!(view_name = %view_name, "View refreshed manually");

        Ok(())
    }

    /// 删除降采样视图
    pub async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP MATERIALIZED VIEW IF EXISTS {} CASCADE", view_name);

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        self.db.execute(stmt).await?;

        info!(view_name = %view_name, "Downsample view dropped");

        Ok(())
    }

    /// 查询降采样视图信息
    pub async fn list_views(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
            SELECT view_name 
            FROM timescaledb_information.continuous_aggregates
            WHERE view_schema = 'public'
        "#;

        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql.to_string());
        let results = self.db.query_all(stmt).await?;

        let mut views = Vec::new();
        for row in results {
            let view_name: String = row.try_get("", "view_name")?;
            views.push(view_name);
        }

        Ok(views)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsample_policy() {
        let policy = DownsamplePolicy::daily();
        assert_eq!(policy.time_bucket_seconds, 86400);
        assert_eq!(policy.target_view, "device_metrics_1d");
    }

    #[test]
    fn test_weekly_policy() {
        let policy = DownsamplePolicy::weekly();
        assert_eq!(policy.time_bucket_seconds, 604800);
    }
}
