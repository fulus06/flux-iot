use super::model::{DeviceCommand, CommandStatus};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 指令队列
#[derive(Clone)]
pub struct CommandQueue {
    /// 按设备 ID 组织的指令队列
    queues: Arc<RwLock<HashMap<String, VecDeque<DeviceCommand>>>>,
    
    /// 所有指令的索引（按指令 ID）
    commands: Arc<RwLock<HashMap<String, DeviceCommand>>>,
    
    /// 最大队列长度
    max_queue_size: usize,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            commands: Arc::new(RwLock::new(HashMap::new())),
            max_queue_size: 100,
        }
    }

    pub fn with_max_size(mut self, max_size: usize) -> Self {
        self.max_queue_size = max_size;
        self
    }

    /// 添加指令到队列
    pub async fn enqueue(&self, command: DeviceCommand) -> anyhow::Result<()> {
        let device_id = command.device_id.clone();
        let command_id = command.id.clone();

        let mut queues = self.queues.write().await;
        let queue = queues.entry(device_id.clone()).or_insert_with(VecDeque::new);

        // 检查队列大小
        if queue.len() >= self.max_queue_size {
            warn!(
                device_id = %device_id,
                queue_size = queue.len(),
                "Command queue is full, dropping oldest command"
            );
            queue.pop_front();
        }

        queue.push_back(command.clone());
        drop(queues);

        // 添加到索引
        self.commands.write().await.insert(command_id.clone(), command);

        info!(
            command_id = %command_id,
            device_id = %device_id,
            "Command enqueued"
        );

        Ok(())
    }

    /// 从队列中取出下一个待执行的指令
    pub async fn dequeue(&self, device_id: &str) -> Option<DeviceCommand> {
        let mut queues = self.queues.write().await;
        let queue = queues.get_mut(device_id)?;

        // 查找第一个待执行的指令
        let pos = queue.iter().position(|cmd| cmd.status == CommandStatus::Pending)?;
        let command = queue.remove(pos)?;

        debug!(
            command_id = %command.id,
            device_id = %device_id,
            "Command dequeued"
        );

        Some(command)
    }

    /// 获取指令
    pub async fn get(&self, command_id: &str) -> Option<DeviceCommand> {
        self.commands.read().await.get(command_id).cloned()
    }

    /// 更新指令状态
    pub async fn update(&self, command: DeviceCommand) -> anyhow::Result<()> {
        let command_id = command.id.clone();
        let device_id = command.device_id.clone();

        // 更新索引
        self.commands.write().await.insert(command_id.clone(), command.clone());

        // 更新队列中的指令
        let mut queues = self.queues.write().await;
        if let Some(queue) = queues.get_mut(&device_id) {
            if let Some(pos) = queue.iter().position(|cmd| cmd.id == command_id) {
                queue[pos] = command;
            }
        }

        Ok(())
    }

    /// 移除指令
    pub async fn remove(&self, command_id: &str) -> Option<DeviceCommand> {
        let command = self.commands.write().await.remove(command_id)?;
        let device_id = command.device_id.clone();

        // 从队列中移除
        let mut queues = self.queues.write().await;
        if let Some(queue) = queues.get_mut(&device_id) {
            if let Some(pos) = queue.iter().position(|cmd| cmd.id == command_id) {
                queue.remove(pos);
            }
        }

        Some(command)
    }

    /// 获取设备的所有指令
    pub async fn get_device_commands(&self, device_id: &str) -> Vec<DeviceCommand> {
        self.queues
            .read()
            .await
            .get(device_id)
            .map(|queue| queue.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 获取待执行的指令数量
    pub async fn pending_count(&self, device_id: &str) -> usize {
        self.queues
            .read()
            .await
            .get(device_id)
            .map(|queue| {
                queue
                    .iter()
                    .filter(|cmd| cmd.status == CommandStatus::Pending)
                    .count()
            })
            .unwrap_or(0)
    }

    /// 清理已完成的指令
    pub async fn cleanup_completed(&self, keep_last: usize) -> usize {
        let mut removed = 0;
        let mut queues = self.queues.write().await;

        for queue in queues.values_mut() {
            let completed: Vec<_> = queue
                .iter()
                .enumerate()
                .filter(|(_, cmd)| cmd.is_completed())
                .map(|(i, _)| i)
                .collect();

            // 保留最后 N 个已完成的指令
            let to_remove = completed.len().saturating_sub(keep_last);
            for &idx in completed.iter().take(to_remove).rev() {
                if let Some(cmd) = queue.remove(idx) {
                    removed += 1;
                    // 同时从索引中移除
                    let commands = self.commands.clone();
                    tokio::spawn(async move {
                        commands.write().await.remove(&cmd.id);
                    });
                }
            }
        }

        if removed > 0 {
            info!(removed = removed, "Cleaned up completed commands");
        }

        removed
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::model::CommandType;

    #[tokio::test]
    async fn test_enqueue_dequeue() {
        let queue = CommandQueue::new();
        let cmd = DeviceCommand::new("device_001".to_string(), CommandType::Reboot);
        let cmd_id = cmd.id.clone();

        queue.enqueue(cmd).await.unwrap();

        let dequeued = queue.dequeue("device_001").await.unwrap();
        assert_eq!(dequeued.id, cmd_id);
    }

    #[tokio::test]
    async fn test_queue_size_limit() {
        let queue = CommandQueue::new().with_max_size(3);

        for i in 0..5 {
            let cmd = DeviceCommand::new(
                "device_001".to_string(),
                CommandType::Custom {
                    name: format!("cmd_{}", i),
                    params: serde_json::Value::Null,
                },
            );
            queue.enqueue(cmd).await.unwrap();
        }

        let commands = queue.get_device_commands("device_001").await;
        assert_eq!(commands.len(), 3);
    }

    #[tokio::test]
    async fn test_update_command() {
        let queue = CommandQueue::new();
        let mut cmd = DeviceCommand::new("device_001".to_string(), CommandType::Reboot);
        let cmd_id = cmd.id.clone();

        queue.enqueue(cmd.clone()).await.unwrap();

        cmd.mark_success(None);
        queue.update(cmd).await.unwrap();

        let updated = queue.get(&cmd_id).await.unwrap();
        assert_eq!(updated.status, CommandStatus::Success);
    }
}
