use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// 设备指令
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceCommand {
    /// 指令 ID
    pub id: String,
    
    /// 设备 ID
    pub device_id: String,
    
    /// 指令类型
    pub command_type: CommandType,
    
    /// 指令参数
    pub params: CommandParams,
    
    /// 超时时间
    #[serde(with = "duration_serde")]
    pub timeout: Duration,
    
    /// 指令状态
    pub status: CommandStatus,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 发送时间
    pub sent_at: Option<DateTime<Utc>>,
    
    /// 执行时间
    pub executed_at: Option<DateTime<Utc>>,
    
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    
    /// 结果
    pub result: Option<serde_json::Value>,
    
    /// 错误信息
    pub error: Option<String>,
}

/// 指令类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum CommandType {
    /// 通用指令
    #[serde(rename = "reboot")]
    Reboot,
    
    #[serde(rename = "reset")]
    Reset,
    
    #[serde(rename = "update")]
    Update,
    
    /// 摄像头指令
    #[serde(rename = "start_stream")]
    StartStream,
    
    #[serde(rename = "stop_stream")]
    StopStream,
    
    #[serde(rename = "take_snapshot")]
    TakeSnapshot,
    
    #[serde(rename = "ptz_control")]
    PTZControl {
        pan: i32,
        tilt: i32,
        zoom: i32,
    },
    
    /// 传感器指令
    #[serde(rename = "read_value")]
    ReadValue,
    
    #[serde(rename = "set_sampling_rate")]
    SetSamplingRate {
        rate: u32,
    },
    
    /// 执行器指令
    #[serde(rename = "set_state")]
    SetState {
        state: bool,
    },
    
    #[serde(rename = "set_value")]
    SetValue {
        value: f64,
    },
    
    /// 自定义指令
    #[serde(rename = "custom")]
    Custom {
        name: String,
        params: serde_json::Value,
    },
}

/// 指令参数
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CommandParams {
    #[serde(flatten)]
    pub data: HashMap<String, serde_json::Value>,
}

impl CommandParams {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert<T: Serialize>(&mut self, key: String, value: T) -> anyhow::Result<()> {
        self.data.insert(key, serde_json::to_value(value)?);
        Ok(())
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> anyhow::Result<Option<T>> {
        match self.data.get(key) {
            Some(value) => Ok(Some(serde_json::from_value(value.clone())?)),
            None => Ok(None),
        }
    }
}

/// 指令状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CommandStatus {
    /// 待发送
    Pending,
    
    /// 已发送
    Sent,
    
    /// 执行中
    Executing,
    
    /// 成功
    Success,
    
    /// 失败
    Failed,
    
    /// 超时
    Timeout,
    
    /// 已取消
    Cancelled,
}

impl DeviceCommand {
    /// 创建新指令
    pub fn new(device_id: String, command_type: CommandType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            device_id,
            command_type,
            params: CommandParams::new(),
            timeout: Duration::from_secs(30),
            status: CommandStatus::Pending,
            created_at: Utc::now(),
            sent_at: None,
            executed_at: None,
            completed_at: None,
            result: None,
            error: None,
        }
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置参数
    pub fn with_params(mut self, params: CommandParams) -> Self {
        self.params = params;
        self
    }

    /// 标记为已发送
    pub fn mark_sent(&mut self) {
        self.status = CommandStatus::Sent;
        self.sent_at = Some(Utc::now());
    }

    /// 标记为执行中
    pub fn mark_executing(&mut self) {
        self.status = CommandStatus::Executing;
        self.executed_at = Some(Utc::now());
    }

    /// 标记为成功
    pub fn mark_success(&mut self, result: Option<serde_json::Value>) {
        self.status = CommandStatus::Success;
        self.completed_at = Some(Utc::now());
        self.result = result;
    }

    /// 标记为失败
    pub fn mark_failed(&mut self, error: String) {
        self.status = CommandStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error);
    }

    /// 标记为超时
    pub fn mark_timeout(&mut self) {
        self.status = CommandStatus::Timeout;
        self.completed_at = Some(Utc::now());
        self.error = Some("Command execution timeout".to_string());
    }

    /// 标记为已取消
    pub fn mark_cancelled(&mut self) {
        self.status = CommandStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    /// 检查是否已完成
    pub fn is_completed(&self) -> bool {
        matches!(
            self.status,
            CommandStatus::Success
                | CommandStatus::Failed
                | CommandStatus::Timeout
                | CommandStatus::Cancelled
        )
    }

    /// 检查是否超时
    pub fn is_timeout(&self) -> bool {
        if let Some(sent_at) = self.sent_at {
            let elapsed = Utc::now().signed_duration_since(sent_at);
            elapsed.num_seconds() as u64 > self.timeout.as_secs()
        } else {
            false
        }
    }
}

// Duration 序列化辅助
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_command() {
        let cmd = DeviceCommand::new(
            "device_001".to_string(),
            CommandType::SetState { state: true },
        );

        assert_eq!(cmd.device_id, "device_001");
        assert_eq!(cmd.status, CommandStatus::Pending);
        assert!(!cmd.is_completed());
    }

    #[test]
    fn test_command_lifecycle() {
        let mut cmd = DeviceCommand::new(
            "device_001".to_string(),
            CommandType::Reboot,
        );

        cmd.mark_sent();
        assert_eq!(cmd.status, CommandStatus::Sent);
        assert!(cmd.sent_at.is_some());

        cmd.mark_executing();
        assert_eq!(cmd.status, CommandStatus::Executing);

        cmd.mark_success(None);
        assert_eq!(cmd.status, CommandStatus::Success);
        assert!(cmd.is_completed());
    }

    #[test]
    fn test_command_params() {
        let mut params = CommandParams::new();
        params.insert("key1".to_string(), "value1").unwrap();
        params.insert("key2".to_string(), 42).unwrap();

        let value1: String = params.get("key1").unwrap().unwrap();
        assert_eq!(value1, "value1");

        let value2: i32 = params.get("key2").unwrap().unwrap();
        assert_eq!(value2, 42);
    }
}
