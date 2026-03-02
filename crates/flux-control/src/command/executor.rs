use super::model::{DeviceCommand, CommandStatus, CommandType};
use super::queue::CommandQueue;
use crate::channel::CommandChannel;
use crate::response::ResponseHandler;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

#[cfg(feature = "persistence")]
use crate::db::CommandRepository;

/// 指令执行器
pub struct CommandExecutor {
    /// 指令队列
    queue: CommandQueue,
    
    /// 指令通道
    channel: Arc<dyn CommandChannel>,
    
    /// 响应处理器
    response_handler: Option<Arc<dyn ResponseHandler>>,
    
    /// 数据库仓库（可选）
    #[cfg(feature = "persistence")]
    repository: Option<Arc<CommandRepository>>,
    
    /// 是否正在运行
    running: Arc<RwLock<bool>>,
}

impl CommandExecutor {
    pub fn new(channel: Arc<dyn CommandChannel>) -> Self {
        Self {
            queue: CommandQueue::new(),
            channel,
            response_handler: None,
            #[cfg(feature = "persistence")]
            repository: None,
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub fn with_queue(mut self, queue: CommandQueue) -> Self {
        self.queue = queue;
        self
    }

    pub fn with_response_handler(mut self, handler: Arc<dyn ResponseHandler>) -> Self {
        self.response_handler = Some(handler);
        self
    }
    
    #[cfg(feature = "persistence")]
    pub fn with_repository(mut self, repository: Arc<CommandRepository>) -> Self {
        self.repository = Some(repository);
        self
    }

    /// 提交指令
    pub async fn submit(&self, command: DeviceCommand) -> anyhow::Result<String> {
        let command_id = command.id.clone();
        self.queue.enqueue(command.clone()).await?;
        
        // 保存到数据库
        #[cfg(feature = "persistence")]
        if let Some(repo) = &self.repository {
            if let Err(e) = repo.save(&command).await {
                warn!(error = %e, "Failed to save command to database");
            }
        }
        
        info!(
            command_id = %command_id,
            "Command submitted"
        );
        
        Ok(command_id)
    }

    /// 执行指令
    pub async fn execute(&self, mut command: DeviceCommand) -> anyhow::Result<()> {
        let command_id = command.id.clone();
        let device_id = command.device_id.clone();
        
        info!(
            command_id = %command_id,
            device_id = %device_id,
            "Executing command"
        );

        // 标记为已发送
        command.mark_sent();
        self.queue.update(command.clone()).await?;

        // 通过通道发送指令
        match self.channel.send_command(&command).await {
            Ok(_) => {
                debug!(
                    command_id = %command_id,
                    "Command sent successfully"
                );
                
                // 等待响应（带超时）
                match timeout(
                    command.timeout,
                    self.wait_for_response(&command_id)
                ).await {
                    Ok(Ok(response)) => {
                        command.mark_success(Some(response));
                        self.queue.update(command.clone()).await?;
                        
                        // 更新数据库
                        #[cfg(feature = "persistence")]
                        if let Some(repo) = &self.repository {
                            if let Err(e) = repo.save(&command).await {
                                warn!(error = %e, "Failed to update command in database");
                            }
                        }
                        
                        if let Some(handler) = &self.response_handler {
                            handler.handle_success(&command).await?;
                        }
                    }
                    Ok(Err(e)) => {
                        command.mark_failed(e.to_string());
                        self.queue.update(command.clone()).await?;
                        
                        // 更新数据库
                        #[cfg(feature = "persistence")]
                        if let Some(repo) = &self.repository {
                            if let Err(e) = repo.save(&command).await {
                                warn!(error = %e, "Failed to update command in database");
                            }
                        }
                        
                        if let Some(handler) = &self.response_handler {
                            handler.handle_failure(&command).await?;
                        }
                    }
                    Err(_) => {
                        warn!(
                            command_id = %command_id,
                            "Command execution timeout"
                        );
                        command.mark_timeout();
                        self.queue.update(command.clone()).await?;
                        
                        if let Some(handler) = &self.response_handler {
                            handler.handle_timeout(&command).await?;
                        }
                    }
                }
            }
            Err(e) => {
                error!(
                    command_id = %command_id,
                    error = %e,
                    "Failed to send command"
                );
                command.mark_failed(e.to_string());
                self.queue.update(command.clone()).await?;
                
                if let Some(handler) = &self.response_handler {
                    handler.handle_failure(&command).await?;
                }
            }
        }

        Ok(())
    }

    /// 等待响应
    async fn wait_for_response(&self, command_id: &str) -> anyhow::Result<serde_json::Value> {
        // 这里应该从响应通道接收响应
        // 简化实现：通过通道订阅响应
        self.channel.wait_response(command_id).await
    }

    /// 取消指令
    pub async fn cancel(&self, command_id: &str) -> anyhow::Result<()> {
        if let Some(mut command) = self.queue.get(command_id).await {
            if !command.is_completed() {
                command.mark_cancelled();
                self.queue.update(command).await?;
                
                info!(
                    command_id = %command_id,
                    "Command cancelled"
                );
            }
        }
        Ok(())
    }

    /// 获取指令状态
    pub async fn get_status(&self, command_id: &str) -> Option<CommandStatus> {
        self.queue.get(command_id).await.map(|cmd| cmd.status)
    }

    /// 获取指令
    pub async fn get_command(&self, command_id: &str) -> Option<DeviceCommand> {
        self.queue.get(command_id).await
    }
    
    /// 查询设备的指令历史
    #[cfg(feature = "persistence")]
    pub async fn list_device_commands(&self, device_id: &str, limit: u64) -> anyhow::Result<Vec<DeviceCommand>> {
        if let Some(repo) = &self.repository {
            let models = repo.find_by_device(device_id, limit).await?;
            
            // 将数据库模型转换为 DeviceCommand
            let commands: Vec<DeviceCommand> = models.into_iter()
                .filter_map(|model| {
                    // 解析 command_type
                    let command_type = match model.command_type.as_str() {
                        s if s.starts_with("SetState") => CommandType::SetState { state: true },
                        "Reboot" => CommandType::Reboot,
                        "Reset" => CommandType::Reset,
                        s if s.starts_with("Custom") => {
                            // 尝试从 params 中提取自定义指令名称
                            let params_value = model.params.clone().unwrap_or_else(|| serde_json::json!({}));
                            let name = params_value.get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            CommandType::Custom { 
                                name, 
                                params: params_value
                            }
                        }
                        _ => return None,
                    };
                    
                    // 解析 status
                    let status = match model.status.as_str() {
                        "Pending" => CommandStatus::Pending,
                        "Sent" => CommandStatus::Sent,
                        "Executing" => CommandStatus::Executing,
                        "Success" => CommandStatus::Success,
                        "Failed" => CommandStatus::Failed,
                        "Timeout" => CommandStatus::Timeout,
                        "Cancelled" => CommandStatus::Cancelled,
                        _ => return None,
                    };
                    
                    // 解析 params
                    let params = model.params
                        .and_then(|p| serde_json::from_value(p).ok())
                        .unwrap_or_default();
                    
                    Some(DeviceCommand {
                        id: model.id,
                        device_id: model.device_id,
                        command_type,
                        params,
                        timeout: Duration::from_secs(model.timeout_seconds as u64),
                        status,
                        created_at: model.created_at,
                        sent_at: model.sent_at,
                        executed_at: model.executed_at,
                        completed_at: model.completed_at,
                        result: model.result,
                        error: model.error,
                    })
                })
                .collect();
            
            Ok(commands)
        } else {
            Ok(vec![])
        }
    }

    /// 启动后台执行器
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        info!("Command executor started");

        // 启动后台任务处理队列
        let queue = self.queue.clone();
        let executor = self.clone_for_background();
        
        tokio::spawn(async move {
            executor.run_background_loop(queue).await;
        });

        Ok(())
    }

    /// 停止执行器
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Command executor stopped");
    }

    /// 后台循环处理队列
    async fn run_background_loop(&self, _queue: CommandQueue) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        
        loop {
            interval.tick().await;
            
            if !*self.running.read().await {
                break;
            }

            // 这里可以实现自动从队列取指令执行
            // 简化版本：由外部调用 execute
        }
    }

    /// 克隆用于后台任务
    fn clone_for_background(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            channel: self.channel.clone(),
            response_handler: self.response_handler.clone(),
            #[cfg(feature = "persistence")]
            repository: self.repository.clone(),
            running: self.running.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::MockCommandChannel;
    use crate::command::model::CommandType;

    #[tokio::test]
    async fn test_submit_command() {
        let channel = Arc::new(MockCommandChannel::new());
        let executor = CommandExecutor::new(channel);

        let cmd = DeviceCommand::new("device_001".to_string(), CommandType::Reboot);
        let cmd_id = executor.submit(cmd).await.unwrap();

        assert!(!cmd_id.is_empty());
    }
}
