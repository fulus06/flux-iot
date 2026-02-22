use crate::model::{EventPoint, LogPoint, MetricPoint};
use crate::query::{AggregatedResult, TimeSeriesQuery};
use async_trait::async_trait;
use sea_orm::{Database, DatabaseConnection, DbErr};
use std::sync::Arc;
use tracing::{debug, info};

/// 时序数据存储 trait
#[async_trait]
pub trait TimeSeriesStore: Send + Sync {
    /// 写入指标数据点
    async fn write_metric(&self, point: &MetricPoint) -> anyhow::Result<()>;
    
    /// 批量写入指标数据点
    async fn write_metrics(&self, points: &[MetricPoint]) -> anyhow::Result<()>;
    
    /// 写入日志数据点
    async fn write_log(&self, point: &LogPoint) -> anyhow::Result<()>;
    
    /// 写入事件数据点
    async fn write_event(&self, point: &EventPoint) -> anyhow::Result<()>;
    
    /// 查询指标数据
    async fn query_metrics(&self, query: &TimeSeriesQuery) -> anyhow::Result<Vec<MetricPoint>>;
    
    /// 查询聚合数据
    async fn query_aggregated(
        &self,
        query: &TimeSeriesQuery,
    ) -> anyhow::Result<Vec<AggregatedResult>>;
}

/// TimescaleDB 存储实现
pub struct TimescaleStore {
    db: Arc<DatabaseConnection>,
}

impl TimescaleStore {
    /// 创建新的 TimescaleDB 存储
    pub async fn new(database_url: &str) -> Result<Self, DbErr> {
        let db = Database::connect(database_url).await?;
        
        info!(
            database_url = %database_url,
            "Connected to TimescaleDB"
        );
        
        Ok(Self {
            db: Arc::new(db),
        })
    }

    /// 获取数据库连接
    pub fn connection(&self) -> &DatabaseConnection {
        &self.db
    }
}

#[async_trait]
impl TimeSeriesStore for TimescaleStore {
    async fn write_metric(&self, point: &MetricPoint) -> anyhow::Result<()> {
        use sea_orm::{sea_query::Query, ConnectionTrait, Statement};

        let sql = format!(
            r#"
            INSERT INTO device_metrics (time, device_id, metric_name, metric_value, unit, tags)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        );

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            vec![
                point.timestamp.into(),
                point.device_id.clone().into(),
                point.metric_name.clone().into(),
                point.metric_value.into(),
                point.unit.clone().into(),
                point.tags.clone().into(),
            ],
        );

        self.db.execute(stmt).await?;

        debug!(
            device_id = %point.device_id,
            metric_name = %point.metric_name,
            "Metric written to TimescaleDB"
        );

        Ok(())
    }

    async fn write_metrics(&self, points: &[MetricPoint]) -> anyhow::Result<()> {
        for point in points {
            self.write_metric(point).await?;
        }
        Ok(())
    }

    async fn write_log(&self, point: &LogPoint) -> anyhow::Result<()> {
        use sea_orm::{ConnectionTrait, Statement};

        let sql = format!(
            r#"
            INSERT INTO device_logs (time, device_id, log_level, message, source, tags)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        );

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            vec![
                point.timestamp.into(),
                point.device_id.clone().into(),
                format!("{:?}", point.log_level).into(),
                point.message.clone().into(),
                point.source.clone().into(),
                point.tags.clone().into(),
            ],
        );

        self.db.execute(stmt).await?;

        debug!(
            device_id = %point.device_id,
            log_level = ?point.log_level,
            "Log written to TimescaleDB"
        );

        Ok(())
    }

    async fn write_event(&self, point: &EventPoint) -> anyhow::Result<()> {
        use sea_orm::{ConnectionTrait, Statement};

        let sql = format!(
            r#"
            INSERT INTO device_events (time, device_id, event_type, event_data, severity)
            VALUES ($1, $2, $3, $4, $5)
            "#
        );

        let severity = point
            .severity
            .as_ref()
            .map(|s| format!("{:?}", s))
            .into();

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            vec![
                point.timestamp.into(),
                point.device_id.clone().into(),
                point.event_type.clone().into(),
                point.event_data.clone().into(),
                severity,
            ],
        );

        self.db.execute(stmt).await?;

        debug!(
            device_id = %point.device_id,
            event_type = %point.event_type,
            "Event written to TimescaleDB"
        );

        Ok(())
    }

    async fn query_metrics(&self, query: &TimeSeriesQuery) -> anyhow::Result<Vec<MetricPoint>> {
        use sea_orm::{ConnectionTrait, Statement};

        let mut sql = String::from(
            "SELECT time, device_id, metric_name, metric_value, unit, tags FROM device_metrics WHERE time >= $1 AND time <= $2"
        );
        
        let mut params: Vec<sea_orm::Value> = vec![
            query.start_time.into(),
            query.end_time.into(),
        ];
        
        let mut param_idx = 3;
        
        if let Some(device_id) = &query.device_id {
            sql.push_str(&format!(" AND device_id = ${}", param_idx));
            params.push(device_id.clone().into());
            param_idx += 1;
        }
        
        if let Some(metric_name) = &query.metric_name {
            sql.push_str(&format!(" AND metric_name = ${}", param_idx));
            params.push(metric_name.clone().into());
            param_idx += 1;
        }
        
        sql.push_str(" ORDER BY time DESC");
        
        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT ${}", param_idx));
            params.push(limit.into());
        }

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            params,
        );

        let results = self.db.query_all(stmt).await?;
        
        let mut points = Vec::new();
        for row in results {
            let point = MetricPoint {
                device_id: row.try_get("", "device_id")?,
                metric_name: row.try_get("", "metric_name")?,
                metric_value: row.try_get("", "metric_value")?,
                unit: row.try_get("", "unit").ok(),
                tags: row.try_get("", "tags").ok(),
                timestamp: row.try_get("", "time")?,
            };
            points.push(point);
        }

        debug!(
            count = points.len(),
            "Queried metrics from TimescaleDB"
        );

        Ok(points)
    }

    async fn query_aggregated(
        &self,
        query: &TimeSeriesQuery,
    ) -> anyhow::Result<Vec<AggregatedResult>> {
        use sea_orm::{ConnectionTrait, Statement};

        if query.aggregation.is_none() || query.time_bucket.is_none() {
            return Err(anyhow::anyhow!("Aggregation type and time bucket are required"));
        }

        let agg_func = match query.aggregation.as_ref().unwrap() {
            crate::query::AggregationType::Avg => "AVG",
            crate::query::AggregationType::Sum => "SUM",
            crate::query::AggregationType::Min => "MIN",
            crate::query::AggregationType::Max => "MAX",
            crate::query::AggregationType::Count => "COUNT",
            crate::query::AggregationType::First => "FIRST",
            crate::query::AggregationType::Last => "LAST",
        };

        let time_bucket = query.time_bucket.unwrap();

        let mut sql = format!(
            "SELECT time_bucket('{}  seconds', time) AS bucket, device_id, metric_name, {}(metric_value) as value, COUNT(*) as count FROM device_metrics WHERE time >= $1 AND time <= $2",
            time_bucket, agg_func
        );
        
        let mut params: Vec<sea_orm::Value> = vec![
            query.start_time.into(),
            query.end_time.into(),
        ];
        
        let mut param_idx = 3;
        
        if let Some(device_id) = &query.device_id {
            sql.push_str(&format!(" AND device_id = ${}", param_idx));
            params.push(device_id.clone().into());
            param_idx += 1;
        }
        
        if let Some(metric_name) = &query.metric_name {
            sql.push_str(&format!(" AND metric_name = ${}", param_idx));
            params.push(metric_name.clone().into());
        }
        
        sql.push_str(" GROUP BY bucket, device_id, metric_name ORDER BY bucket DESC");

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            params,
        );

        let results = self.db.query_all(stmt).await?;
        
        let mut aggregated = Vec::new();
        for row in results {
            let result = AggregatedResult {
                bucket: row.try_get("", "bucket")?,
                device_id: row.try_get("", "device_id")?,
                metric_name: row.try_get("", "metric_name")?,
                value: row.try_get("", "value")?,
                count: row.try_get("", "count").ok(),
            };
            aggregated.push(result);
        }

        debug!(
            count = aggregated.len(),
            "Queried aggregated metrics from TimescaleDB"
        );

        Ok(aggregated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metric_point_creation() {
        let point = MetricPoint::new(
            "device_001".to_string(),
            "temperature".to_string(),
            25.5,
        );

        assert_eq!(point.device_id, "device_001");
        assert_eq!(point.metric_value, 25.5);
    }
}
