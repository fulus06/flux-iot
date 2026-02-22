use crate::command::model::DeviceCommand;
use async_trait::async_trait;

/// 指令通道 trait
#[async_trait]
pub trait CommandChannel: Send + Sync {
    /// 发送指令到设备
    async fn send_command(&self, command: &DeviceCommand) -> anyhow::Result<()>;
    
    /// 等待指令响应
    async fn wait_response(&self, command_id: &str) -> anyhow::Result<serde_json::Value>;
    
    /// 订阅设备响应
    async fn subscribe_device(&self, device_id: &str) -> anyhow::Result<()>;
    
    /// 取消订阅设备响应
    async fn unsubscribe_device(&self, device_id: &str) -> anyhow::Result<()>;
}

#[cfg(test)]
pub struct MockCommandChannel;

#[cfg(test)]
impl MockCommandChannel {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
#[async_trait]
impl CommandChannel for MockCommandChannel {
    async fn send_command(&self, _command: &DeviceCommand) -> anyhow::Result<()> {
        Ok(())
    }
    
    async fn wait_response(&self, _command_id: &str) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({"status": "success"}))
    }
    
    async fn subscribe_device(&self, _device_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    
    async fn unsubscribe_device(&self, _device_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
