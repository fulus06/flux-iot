use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 时序数据查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesQuery {
    /// 设备 ID
    pub device_id: Option<String>,
    
    /// 指标名称
    pub metric_name: Option<String>,
    
    /// 开始时间
    pub start_time: DateTime<Utc>,
    
    /// 结束时间
    pub end_time: DateTime<Utc>,
    
    /// 聚合类型
    pub aggregation: Option<AggregationType>,
    
    /// 聚合时间窗口（秒）
    pub time_bucket: Option<i64>,
    
    /// 限制返回数量
    pub limit: Option<i64>,
}

/// 聚合类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AggregationType {
    Avg,
    Sum,
    Min,
    Max,
    Count,
    First,
    Last,
}

impl TimeSeriesQuery {
    pub fn new(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Self {
        Self {
            device_id: None,
            metric_name: None,
            start_time,
            end_time,
            aggregation: None,
            time_bucket: None,
            limit: None,
        }
    }

    pub fn with_device(mut self, device_id: String) -> Self {
        self.device_id = Some(device_id);
        self
    }

    pub fn with_metric(mut self, metric_name: String) -> Self {
        self.metric_name = Some(metric_name);
        self
    }

    pub fn with_aggregation(mut self, aggregation: AggregationType, time_bucket: i64) -> Self {
        self.aggregation = Some(aggregation);
        self.time_bucket = Some(time_bucket);
        self
    }

    pub fn with_limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// 聚合结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResult {
    pub bucket: DateTime<Utc>,
    pub device_id: String,
    pub metric_name: String,
    pub value: f64,
    pub count: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_query_builder() {
        let now = Utc::now();
        let query = TimeSeriesQuery::new(now - Duration::hours(1), now)
            .with_device("device_001".to_string())
            .with_metric("temperature".to_string())
            .with_aggregation(AggregationType::Avg, 300)
            .with_limit(100);

        assert_eq!(query.device_id, Some("device_001".to_string()));
        assert_eq!(query.aggregation, Some(AggregationType::Avg));
        assert_eq!(query.time_bucket, Some(300));
    }
}
