use crate::command::model::DeviceCommand;
use async_trait::async_trait;

/// 响应处理器 trait
#[async_trait]
pub trait ResponseHandler: Send + Sync {
    /// 处理成功响应
    async fn handle_success(&self, command: &DeviceCommand) -> anyhow::Result<()>;
    
    /// 处理失败响应
    async fn handle_failure(&self, command: &DeviceCommand) -> anyhow::Result<()>;
    
    /// 处理超时
    async fn handle_timeout(&self, command: &DeviceCommand) -> anyhow::Result<()>;
}

/// 默认响应处理器
pub struct DefaultResponseHandler;

impl DefaultResponseHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultResponseHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ResponseHandler for DefaultResponseHandler {
    async fn handle_success(&self, command: &DeviceCommand) -> anyhow::Result<()> {
        tracing::info!(
            command_id = %command.id,
            device_id = %command.device_id,
            "Command executed successfully"
        );
        Ok(())
    }
    
    async fn handle_failure(&self, command: &DeviceCommand) -> anyhow::Result<()> {
        tracing::error!(
            command_id = %command.id,
            device_id = %command.device_id,
            error = ?command.error,
            "Command execution failed"
        );
        Ok(())
    }
    
    async fn handle_timeout(&self, command: &DeviceCommand) -> anyhow::Result<()> {
        tracing::warn!(
            command_id = %command.id,
            device_id = %command.device_id,
            "Command execution timeout"
        );
        Ok(())
    }
}
