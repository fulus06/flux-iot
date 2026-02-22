use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// 规则执行上下文
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleContext {
    /// 设备数据
    pub device_data: HashMap<String, Value>,
    
    /// 系统变量
    pub system_vars: HashMap<String, Value>,
    
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    
    /// 触发事件信息
    pub trigger_info: Option<TriggerInfo>,
}

/// 触发信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInfo {
    pub trigger_type: String,
    pub device_id: Option<String>,
    pub event_type: Option<String>,
    pub metric: Option<String>,
}

impl RuleContext {
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            ..Default::default()
        }
    }
    
    pub fn with_device_data(mut self, data: HashMap<String, Value>) -> Self {
        self.device_data = data;
        self
    }
}
