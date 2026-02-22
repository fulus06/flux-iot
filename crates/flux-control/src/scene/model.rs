use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 场景定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// 场景 ID
    pub id: String,
    
    /// 场景名称
    pub name: String,
    
    /// 场景描述
    pub description: Option<String>,
    
    /// 触发器列表
    pub triggers: Vec<SceneTrigger>,
    
    /// 条件脚本（Rhai）- 可选
    /// 返回 true 时执行动作
    pub condition_script: Option<String>,
    
    /// 动作脚本（Rhai）
    pub action_script: String,
    
    /// 是否启用
    pub enabled: bool,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 场景触发器
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum SceneTrigger {
    /// 手动触发
    #[serde(rename = "manual")]
    Manual,
    
    /// 定时触发（Cron 表达式）
    #[serde(rename = "schedule")]
    Schedule {
        cron: String,
    },
    
    /// 设备事件触发
    #[serde(rename = "device_event")]
    DeviceEvent {
        device_id: String,
        event_type: String,
    },
    
    /// 设备指标变化触发
    #[serde(rename = "metric_change")]
    MetricChange {
        device_id: String,
        metric: String,
        operator: ComparisonOperator,
        threshold: f64,
    },
    
    /// 设备状态变化触发
    #[serde(rename = "status_change")]
    StatusChange {
        device_id: String,
        from_status: Option<String>,
        to_status: String,
    },
}

/// 比较操作符
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    Equal,
    GreaterOrEqual,
    LessOrEqual,
    NotEqual,
}

/// 场景条件（用于简单场景，复杂场景使用脚本）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCondition {
    pub device_id: String,
    pub condition_type: ConditionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConditionType {
    DeviceStatus { status: String },
    MetricThreshold { metric: String, operator: ComparisonOperator, value: f64 },
    TimeRange { start_hour: u8, end_hour: u8 },
}

/// 场景动作（用于简单场景，复杂场景使用脚本）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneAction {
    pub device_id: String,
    pub command_type: String,
    pub params: serde_json::Value,
    pub delay_seconds: Option<u64>,
}

/// 场景执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneExecution {
    pub id: i64,
    pub scene_id: String,
    pub trigger_type: String,
    pub executed_at: DateTime<Utc>,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
}

impl Scene {
    pub fn new(name: String, action_script: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description: None,
            triggers: vec![],
            condition_script: None,
            action_script,
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn with_trigger(mut self, trigger: SceneTrigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    pub fn with_condition(mut self, condition_script: String) -> Self {
        self.condition_script = Some(condition_script);
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_scene() {
        let scene = Scene::new(
            "温度控制".to_string(),
            r#"send_command("fan_01", "set_state", #{state: true})"#.to_string(),
        )
        .with_trigger(SceneTrigger::MetricChange {
            device_id: "sensor_01".to_string(),
            metric: "temperature".to_string(),
            operator: ComparisonOperator::GreaterThan,
            threshold: 30.0,
        })
        .with_condition(r#"get_metric("sensor_01", "temperature") > 30.0"#.to_string());

        assert_eq!(scene.name, "温度控制");
        assert_eq!(scene.triggers.len(), 1);
        assert!(scene.condition_script.is_some());
        assert!(scene.enabled);
    }

    #[test]
    fn test_scene_serialization() {
        let scene = Scene::new(
            "测试场景".to_string(),
            "print('test')".to_string(),
        );

        let json = serde_json::to_string(&scene).unwrap();
        let deserialized: Scene = serde_json::from_str(&json).unwrap();

        assert_eq!(scene.name, deserialized.name);
        assert_eq!(scene.action_script, deserialized.action_script);
    }
}
