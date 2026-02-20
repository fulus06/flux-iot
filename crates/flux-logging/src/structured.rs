use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<tracing::Level> for LogLevel {
    fn from(level: tracing::Level) -> Self {
        match level {
            tracing::Level::TRACE => LogLevel::Trace,
            tracing::Level::DEBUG => LogLevel::Debug,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::ERROR => LogLevel::Error,
        }
    }
}

/// 结构化日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    
    /// 日志级别
    pub level: LogLevel,
    
    /// 日志消息
    pub message: String,
    
    /// 日志目标（模块路径）
    pub target: String,
    
    /// 追踪 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    
    /// Span ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    
    /// 父 Span ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    
    /// 服务名称
    pub service_name: String,
    
    /// 主机名
    pub host: String,
    
    /// 环境（dev/staging/production）
    pub environment: String,
    
    /// 自定义字段
    #[serde(flatten)]
    pub fields: HashMap<String, serde_json::Value>,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: String, target: String) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            message,
            target,
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            service_name: "flux-iot".to_string(),
            host: hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string()),
            environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            fields: HashMap::new(),
        }
    }

    pub fn with_trace_id(mut self, trace_id: String) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn with_span_id(mut self, span_id: String) -> Self {
        self.span_id = Some(span_id);
        self
    }

    pub fn with_parent_span_id(mut self, parent_span_id: String) -> Self {
        self.parent_span_id = Some(parent_span_id);
        self
    }

    pub fn with_field(mut self, key: String, value: serde_json::Value) -> Self {
        self.fields.insert(key, value);
        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// 结构化日志构建器
pub struct LogEntryBuilder {
    entry: LogEntry,
}

impl LogEntryBuilder {
    pub fn new(level: LogLevel, message: String) -> Self {
        Self {
            entry: LogEntry::new(level, message, "unknown".to_string()),
        }
    }

    pub fn target(mut self, target: String) -> Self {
        self.entry.target = target;
        self
    }

    pub fn trace_id(mut self, trace_id: String) -> Self {
        self.entry.trace_id = Some(trace_id);
        self
    }

    pub fn span_id(mut self, span_id: String) -> Self {
        self.entry.span_id = Some(span_id);
        self
    }

    pub fn field(mut self, key: String, value: serde_json::Value) -> Self {
        self.entry.fields.insert(key, value);
        self
    }

    pub fn build(self) -> LogEntry {
        self.entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "test::module".to_string(),
        );

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.target, "test::module");
    }

    #[test]
    fn test_log_entry_with_trace() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "test::module".to_string(),
        )
        .with_trace_id("trace-123".to_string())
        .with_span_id("span-456".to_string());

        assert_eq!(entry.trace_id, Some("trace-123".to_string()));
        assert_eq!(entry.span_id, Some("span-456".to_string()));
    }

    #[test]
    fn test_log_entry_json() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "test::module".to_string(),
        )
        .with_field("user_id".to_string(), serde_json::json!("user-123"));

        let json = entry.to_json().unwrap();
        assert!(json.contains("Test message"));
        assert!(json.contains("user-123"));
    }

    #[test]
    fn test_log_entry_builder() {
        let entry = LogEntryBuilder::new(LogLevel::Info, "Test".to_string())
            .target("test".to_string())
            .trace_id("trace-123".to_string())
            .field("key".to_string(), serde_json::json!("value"))
            .build();

        assert_eq!(entry.target, "test");
        assert_eq!(entry.trace_id, Some("trace-123".to_string()));
    }
}
