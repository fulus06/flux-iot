use crate::command::model::{CommandStatus, CommandType, DeviceCommand};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 批量指令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCommand {
    /// 批量指令 ID
    pub id: String,
    
    /// 批量指令名称
    pub name: Option<String>,
    
    /// 目标设备 ID 列表
    pub device_ids: Vec<String>,
    
    /// 指令类型
    pub command_type: CommandType,
    
    /// 指令参数（所有设备共享）
    pub params: serde_json::Value,
    
    /// 并发数（同时执行的指令数）
    pub concurrency: usize,
    
    /// 失败是否继续
    pub continue_on_error: bool,
    
    /// 超时时间（秒）
    pub timeout_seconds: u64,
    
    /// 批量指令状态
    pub status: BatchStatus,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 开始时间
    pub started_at: Option<DateTime<Utc>>,
    
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
}

/// 批量指令状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BatchStatus {
    /// 待执行
    Pending,
    
    /// 执行中
    Running,
    
    /// 已完成
    Completed,
    
    /// 部分失败
    PartialFailure,
    
    /// 全部失败
    Failed,
    
    /// 已取消
    Cancelled,
}

/// 批量执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    /// 批量指令 ID
    pub batch_id: String,
    
    /// 总设备数
    pub total: usize,
    
    /// 成功数
    pub success: usize,
    
    /// 失败数
    pub failed: usize,
    
    /// 超时数
    pub timeout: usize,
    
    /// 已取消数
    pub cancelled: usize,
    
    /// 每个设备的执行结果
    pub results: Vec<CommandResult>,
    
    /// 执行时长（毫秒）
    pub duration_ms: Option<i64>,
}

/// 单个设备的指令结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// 设备 ID
    pub device_id: String,
    
    /// 指令 ID
    pub command_id: String,
    
    /// 执行状态
    pub status: CommandStatus,
    
    /// 结果数据
    pub result: Option<serde_json::Value>,
    
    /// 错误信息
    pub error: Option<String>,
    
    /// 执行时长（毫秒）
    pub duration_ms: Option<i64>,
}

impl BatchCommand {
    pub fn new(device_ids: Vec<String>, command_type: CommandType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: None,
            device_ids,
            command_type,
            params: serde_json::Value::Null,
            concurrency: 10, // 默认并发数
            continue_on_error: true,
            timeout_seconds: 30,
            status: BatchStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1); // 至少为 1
        self
    }

    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    /// 标记为运行中
    pub fn mark_running(&mut self) {
        self.status = BatchStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// 标记为已完成
    pub fn mark_completed(&mut self, result: &BatchResult) {
        self.completed_at = Some(Utc::now());
        
        if result.failed == 0 && result.timeout == 0 {
            self.status = BatchStatus::Completed;
        } else if result.success == 0 {
            self.status = BatchStatus::Failed;
        } else {
            self.status = BatchStatus::PartialFailure;
        }
    }

    /// 标记为已取消
    pub fn mark_cancelled(&mut self) {
        self.status = BatchStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }
}

impl BatchResult {
    pub fn new(batch_id: String, total: usize) -> Self {
        Self {
            batch_id,
            total,
            success: 0,
            failed: 0,
            timeout: 0,
            cancelled: 0,
            results: Vec::with_capacity(total),
            duration_ms: None,
        }
    }

    /// 添加指令结果
    pub fn add_result(&mut self, result: CommandResult) {
        match result.status {
            CommandStatus::Success => self.success += 1,
            CommandStatus::Failed => self.failed += 1,
            CommandStatus::Timeout => self.timeout += 1,
            CommandStatus::Cancelled => self.cancelled += 1,
            _ => {}
        }
        self.results.push(result);
    }

    /// 计算成功率
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.success as f64 / self.total as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_batch_command() {
        let devices = vec!["device_001".to_string(), "device_002".to_string()];
        let batch = BatchCommand::new(devices.clone(), CommandType::Reboot)
            .with_name("重启所有设备".to_string())
            .with_concurrency(5);

        assert_eq!(batch.device_ids.len(), 2);
        assert_eq!(batch.concurrency, 5);
        assert_eq!(batch.status, BatchStatus::Pending);
    }

    #[test]
    fn test_batch_result() {
        let mut result = BatchResult::new("batch_001".to_string(), 3);

        result.add_result(CommandResult {
            device_id: "device_001".to_string(),
            command_id: "cmd_001".to_string(),
            status: CommandStatus::Success,
            result: None,
            error: None,
            duration_ms: Some(100),
        });

        result.add_result(CommandResult {
            device_id: "device_002".to_string(),
            command_id: "cmd_002".to_string(),
            status: CommandStatus::Failed,
            result: None,
            error: Some("Error".to_string()),
            duration_ms: Some(50),
        });

        assert_eq!(result.success, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.results.len(), 2);
        assert!((result.success_rate() - 33.33).abs() < 0.1);
    }
}
