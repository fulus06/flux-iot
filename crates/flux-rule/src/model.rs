use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 规则定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// 规则 ID
    pub id: String,
    
    /// 规则名称
    pub name: String,
    
    /// 规则描述
    pub description: String,
    
    /// 规则分组
    pub group: Option<String>,
    
    /// 规则标签
    pub tags: Vec<String>,
    
    /// 是否启用
    pub enabled: bool,
    
    /// 触发器
    pub trigger: RuleTrigger,
    
    /// Rhai 脚本
    pub script: String,
    
    /// 优先级（1-100，数字越大优先级越高）
    pub priority: i32,
    
    /// 冲突策略
    pub conflict_strategy: ConflictStrategy,
    
    /// 执行超时（秒）
    pub timeout_seconds: u64,
    
    /// 限流配置
    pub rate_limit: Option<RateLimit>,
    
    /// 规则参数
    pub parameters: HashMap<String, serde_json::Value>,
    
    /// 测试模式
    pub test_mode: bool,
    
    /// 执行成功时通知
    pub notification_on_success: bool,
    
    /// 执行失败时通知
    pub notification_on_failure: bool,
    
    /// 通知渠道
    pub notification_channels: Vec<String>,
    
    /// 版本号
    pub version: i32,
    
    /// 上一版本 ID
    pub previous_version: Option<String>,
    
    /// 规则依赖
    pub dependencies: Vec<String>,
    
    /// 元数据
    pub metadata: RuleMetadata,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            description: String::new(),
            group: None,
            tags: Vec::new(),
            enabled: true,
            trigger: RuleTrigger::Manual,
            script: String::new(),
            priority: 50,
            conflict_strategy: ConflictStrategy::Parallel,
            timeout_seconds: 30,
            rate_limit: None,
            parameters: HashMap::new(),
            test_mode: false,
            notification_on_success: false,
            notification_on_failure: true,
            notification_channels: Vec::new(),
            version: 1,
            previous_version: None,
            dependencies: Vec::new(),
            metadata: RuleMetadata::default(),
        }
    }
}

/// 触发器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleTrigger {
    /// 手动触发
    Manual,
    
    /// 定时触发
    Schedule {
        /// Cron 表达式
        cron: String,
    },
    
    /// 条件触发 - 设备事件
    DeviceEvent {
        /// 设备 ID
        device_id: String,
        /// 事件类型
        event_type: String,
    },
    
    /// 条件触发 - 数据变化
    DataChange {
        /// 设备 ID
        device_id: String,
        /// 指标名称（None 表示任何指标）
        metric: Option<String>,
    },
}

/// 冲突策略
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConflictStrategy {
    /// 并行执行
    Parallel,
    
    /// 按优先级顺序执行
    Sequential,
    
    /// 互斥执行（同组只执行一个）
    Exclusive {
        group: String,
    },
}

/// 限流配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// 最大执行次数
    pub max_executions: u32,
    
    /// 时间窗口（秒）
    pub time_window_seconds: u64,
}

/// 规则元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMetadata {
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    
    /// 创建者
    pub created_by: String,
    
    /// 最后执行时间
    pub last_executed_at: Option<DateTime<Utc>>,
    
    /// 执行次数
    pub execution_count: u64,
    
    /// 成功次数
    pub success_count: u64,
    
    /// 失败次数
    pub failure_count: u64,
}

impl Default for RuleMetadata {
    fn default() -> Self {
        Self {
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: "system".to_string(),
            last_executed_at: None,
            execution_count: 0,
            success_count: 0,
            failure_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_default() {
        let rule = Rule::default();
        assert_eq!(rule.priority, 50);
        assert_eq!(rule.timeout_seconds, 30);
        assert!(rule.enabled);
    }

    #[test]
    fn test_rule_serialization() {
        let rule = Rule {
            name: "test_rule".to_string(),
            trigger: RuleTrigger::Manual,
            ..Default::default()
        };
        
        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: Rule = serde_json::from_str(&json).unwrap();
        
        assert_eq!(rule.name, deserialized.name);
    }
}
