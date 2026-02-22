use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 通用数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub measurement: String,
    pub tags: HashMap<String, String>,
    pub fields: HashMap<String, serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

/// 设备指标数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub device_id: String,
    pub metric_name: String,
    pub metric_value: f64,
    pub unit: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

impl MetricPoint {
    pub fn new(device_id: String, metric_name: String, metric_value: f64) -> Self {
        Self {
            device_id,
            metric_name,
            metric_value,
            unit: None,
            tags: None,
            timestamp: Utc::now(),
        }
    }

    pub fn with_unit(mut self, unit: String) -> Self {
        self.unit = Some(unit);
        self
    }

    pub fn with_tags(mut self, tags: serde_json::Value) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }
}

/// 设备日志数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPoint {
    pub device_id: String,
    pub log_level: LogLevel,
    pub message: String,
    pub source: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogPoint {
    pub fn new(device_id: String, log_level: LogLevel, message: String) -> Self {
        Self {
            device_id,
            log_level,
            message,
            source: None,
            tags: None,
            timestamp: Utc::now(),
        }
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_tags(mut self, tags: serde_json::Value) -> Self {
        self.tags = Some(tags);
        self
    }
}

/// 设备事件数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPoint {
    pub device_id: String,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub severity: Option<EventSeverity>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl EventPoint {
    pub fn new(device_id: String, event_type: String, event_data: serde_json::Value) -> Self {
        Self {
            device_id,
            event_type,
            event_data,
            severity: None,
            timestamp: Utc::now(),
        }
    }

    pub fn with_severity(mut self, severity: EventSeverity) -> Self {
        self.severity = Some(severity);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_point() {
        let point = MetricPoint::new(
            "device_001".to_string(),
            "temperature".to_string(),
            25.5,
        )
        .with_unit("celsius".to_string());

        assert_eq!(point.device_id, "device_001");
        assert_eq!(point.metric_value, 25.5);
        assert_eq!(point.unit, Some("celsius".to_string()));
    }

    #[test]
    fn test_log_point() {
        let point = LogPoint::new(
            "device_001".to_string(),
            LogLevel::Info,
            "Device started".to_string(),
        );

        assert_eq!(point.log_level, LogLevel::Info);
        assert_eq!(point.message, "Device started");
    }
}
