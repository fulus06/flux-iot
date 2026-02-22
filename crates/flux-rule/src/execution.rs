use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 规则执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleExecution {
    pub id: String,
    pub rule_id: String,
    pub rule_name: String,
    pub trigger_type: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,
    pub error: Option<String>,
    pub context: Value,
}

/// 执行状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
    Timeout,
}

/// 测试结果
#[derive(Debug, Clone)]
pub struct TestResult {
    pub success: bool,
    pub logs: Vec<String>,
    pub actions: Vec<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}
