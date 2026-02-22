use super::model::{BatchCommand, BatchResult, CommandResult};
use crate::command::{CommandExecutor, CommandStatus, DeviceCommand};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

/// 批量执行器
pub struct BatchExecutor {
    /// 指令执行器
    command_executor: Arc<CommandExecutor>,
}

impl BatchExecutor {
    pub fn new(command_executor: Arc<CommandExecutor>) -> Self {
        Self { command_executor }
    }

    /// 执行批量指令
    pub async fn execute(&self, mut batch: BatchCommand) -> anyhow::Result<BatchResult> {
        let start_time = Instant::now();
        batch.mark_running();

        info!(
            batch_id = %batch.id,
            device_count = batch.device_ids.len(),
            concurrency = batch.concurrency,
            "Starting batch command execution"
        );

        let mut result = BatchResult::new(batch.id.clone(), batch.device_ids.len());

        // 创建信号量控制并发
        let semaphore = Arc::new(Semaphore::new(batch.concurrency));
        let mut tasks = Vec::new();

        for device_id in &batch.device_ids {
            let device_id = device_id.clone();
            let command_type = batch.command_type.clone();
            let params = batch.params.clone();
            let timeout = Duration::from_secs(batch.timeout_seconds);
            let executor = self.command_executor.clone();
            let semaphore = semaphore.clone();
            let continue_on_error = batch.continue_on_error;

            let task = tokio::spawn(async move {
                // 获取信号量许可
                let _permit = semaphore.acquire().await.unwrap();

                let task_start = Instant::now();
                
                // 创建指令
                let command = DeviceCommand::new(device_id.clone(), command_type)
                    .with_timeout(timeout);

                let command_id = command.id.clone();

                debug!(
                    device_id = %device_id,
                    command_id = %command_id,
                    "Executing batch command for device"
                );

                // 执行指令
                match executor.submit(command.clone()).await {
                    Ok(_) => {
                        match executor.execute(command).await {
                            Ok(_) => {
                                // 获取最终状态
                                let final_status = executor
                                    .get_status(&command_id)
                                    .await
                                    .unwrap_or(CommandStatus::Success);

                                let final_command = executor.get_command(&command_id).await;

                                CommandResult {
                                    device_id,
                                    command_id,
                                    status: final_status,
                                    result: final_command.as_ref().and_then(|c| c.result.clone()),
                                    error: final_command.as_ref().and_then(|c| c.error.clone()),
                                    duration_ms: Some(task_start.elapsed().as_millis() as i64),
                                }
                            }
                            Err(e) => {
                                error!(
                                    device_id = %device_id,
                                    error = %e,
                                    "Failed to execute command"
                                );
                                CommandResult {
                                    device_id,
                                    command_id,
                                    status: CommandStatus::Failed,
                                    result: None,
                                    error: Some(e.to_string()),
                                    duration_ms: Some(task_start.elapsed().as_millis() as i64),
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            device_id = %device_id,
                            error = %e,
                            "Failed to submit command"
                        );
                        CommandResult {
                            device_id,
                            command_id,
                            status: CommandStatus::Failed,
                            result: None,
                            error: Some(e.to_string()),
                            duration_ms: Some(task_start.elapsed().as_millis() as i64),
                        }
                    }
                }
            });

            tasks.push(task);

            // 如果不继续执行且有失败，提前退出
            if !continue_on_error && result.failed > 0 {
                break;
            }
        }

        // 等待所有任务完成
        for task in tasks {
            match task.await {
                Ok(cmd_result) => {
                    result.add_result(cmd_result);
                }
                Err(e) => {
                    error!(error = %e, "Task join error");
                }
            }
        }

        result.duration_ms = Some(start_time.elapsed().as_millis() as i64);
        batch.mark_completed(&result);

        info!(
            batch_id = %batch.id,
            total = result.total,
            success = result.success,
            failed = result.failed,
            duration_ms = ?result.duration_ms,
            "Batch command execution completed"
        );

        Ok(result)
    }

    /// 取消批量指令（简化实现）
    pub async fn cancel(&self, batch_id: &str) -> anyhow::Result<()> {
        info!(batch_id = %batch_id, "Batch command cancelled");
        // TODO: 实现实际的取消逻辑
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::MockCommandChannel;
    use crate::command::CommandType;

    #[tokio::test]
    async fn test_batch_executor() {
        let channel = Arc::new(MockCommandChannel::new());
        let executor = Arc::new(CommandExecutor::new(channel));
        let batch_executor = BatchExecutor::new(executor);

        let devices = vec![
            "device_001".to_string(),
            "device_002".to_string(),
            "device_003".to_string(),
        ];

        let batch = BatchCommand::new(devices, CommandType::Reboot)
            .with_concurrency(2);

        let result = batch_executor.execute(batch).await.unwrap();

        assert_eq!(result.total, 3);
        assert_eq!(result.results.len(), 3);
    }

    #[tokio::test]
    async fn test_batch_concurrency() {
        let channel = Arc::new(MockCommandChannel::new());
        let executor = Arc::new(CommandExecutor::new(channel));
        let batch_executor = BatchExecutor::new(executor);

        let devices: Vec<String> = (0..10).map(|i| format!("device_{:03}", i)).collect();

        let batch = BatchCommand::new(devices, CommandType::Reboot)
            .with_concurrency(3);

        let start = Instant::now();
        let result = batch_executor.execute(batch).await.unwrap();
        let duration = start.elapsed();

        assert_eq!(result.total, 10);
        // 并发执行应该比串行快
        assert!(duration.as_secs() < 10);
    }
}
