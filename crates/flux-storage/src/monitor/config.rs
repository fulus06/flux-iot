use crate::pool::PoolConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 监控服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// 监控间隔（秒）
    pub check_interval_secs: u64,
    
    /// 告警去重间隔（分钟）
    pub alert_dedup_minutes: i64,

    /// 是否发送恢复通知（从告警状态恢复为 Healthy）
    #[serde(default = "default_true")]
    pub send_recovery_alerts: bool,

    /// 是否仅在状态变化时发送告警
    #[serde(default = "default_true")]
    pub alert_on_status_change: bool,

    #[serde(default)]
    pub enable_lifecycle_gc: bool,

    #[serde(default = "default_retention_days")]
    pub retention_days: u64,

    #[serde(default)]
    pub max_capacity_gb: Option<u64>,

    #[serde(default = "default_gc_trigger_usage_percent")]
    pub gc_trigger_usage_percent: f64,

    #[serde(default = "default_gc_target_usage_percent")]
    pub gc_target_usage_percent: f64,

    #[serde(default)]
    pub notify_gc_results: bool,

    #[serde(default)]
    pub telemetry_enabled: bool,

    #[serde(default)]
    pub telemetry_endpoint: Option<String>,

    #[serde(default = "default_telemetry_timeout_ms")
    ]
    pub telemetry_timeout_ms: u64,
    
    /// 存储池配置
    pub storage_pools: Vec<PoolConfig>,
}

fn default_true() -> bool {
    true
}

fn default_retention_days() -> u64 {
    7
}

fn default_gc_trigger_usage_percent() -> f64 {
    90.0
}

fn default_gc_target_usage_percent() -> f64 {
    80.0
}

fn default_telemetry_timeout_ms() -> u64 {
    1000
}

impl MonitorConfig {
    /// 从文件加载配置
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 60,
            alert_dedup_minutes: 5,
            send_recovery_alerts: true,
            alert_on_status_change: true,
            enable_lifecycle_gc: false,
            retention_days: 7,
            max_capacity_gb: None,
            gc_trigger_usage_percent: 90.0,
            gc_target_usage_percent: 80.0,
            notify_gc_results: false,
            telemetry_enabled: false,
            telemetry_endpoint: None,
            telemetry_timeout_ms: 1000,
            storage_pools: Vec::new(),
        }
    }
}
