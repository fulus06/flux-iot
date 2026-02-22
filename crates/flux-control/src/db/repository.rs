use crate::command::model::{DeviceCommand, CommandStatus};
use crate::db::entities::{self, Model as CommandModel};
use chrono::Utc;
use sea_orm::{entity::prelude::*, DatabaseConnection, QueryOrder, QuerySelect, Set};
use std::sync::Arc;
use tracing::{debug, info};

/// 指令仓库
pub struct CommandRepository {
    db: Arc<DatabaseConnection>,
}

impl CommandRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 保存指令
    pub async fn save(&self, command: &DeviceCommand) -> anyhow::Result<()> {
        let model = entities::ActiveModel {
            id: Set(command.id.clone()),
            device_id: Set(command.device_id.clone()),
            command_type: Set(format!("{:?}", command.command_type)),
            params: Set(Some(serde_json::to_value(&command.params)?)),
            timeout_seconds: Set(command.timeout.as_secs() as i32),
            status: Set(format!("{:?}", command.status)),
            created_at: Set(command.created_at),
            sent_at: Set(command.sent_at),
            executed_at: Set(command.executed_at),
            completed_at: Set(command.completed_at),
            result: Set(command.result.clone()),
            error: Set(command.error.clone()),
        };

        entities::Entity::insert(model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(entities::Column::Id)
                    .update_columns([
                        entities::Column::Status,
                        entities::Column::SentAt,
                        entities::Column::ExecutedAt,
                        entities::Column::CompletedAt,
                        entities::Column::Result,
                        entities::Column::Error,
                    ])
                    .to_owned(),
            )
            .exec(&*self.db)
            .await?;

        debug!(command_id = %command.id, "Command saved to database");
        Ok(())
    }

    /// 根据 ID 查询指令
    pub async fn find_by_id(&self, command_id: &str) -> anyhow::Result<Option<CommandModel>> {
        let model = entities::Entity::find_by_id(command_id.to_string())
            .one(&*self.db)
            .await?;

        Ok(model)
    }

    /// 查询设备的指令历史
    pub async fn find_by_device(
        &self,
        device_id: &str,
        limit: u64,
    ) -> anyhow::Result<Vec<CommandModel>> {
        let models = entities::Entity::find()
            .filter(entities::Column::DeviceId.eq(device_id))
            .order_by_desc(entities::Column::CreatedAt)
            .limit(limit)
            .all(&*self.db)
            .await?;

        Ok(models)
    }

    /// 查询指定状态的指令
    pub async fn find_by_status(
        &self,
        status: CommandStatus,
        limit: u64,
    ) -> anyhow::Result<Vec<CommandModel>> {
        let status_str = format!("{:?}", status);
        
        let models = entities::Entity::find()
            .filter(entities::Column::Status.eq(status_str))
            .order_by_desc(entities::Column::CreatedAt)
            .limit(limit)
            .all(&*self.db)
            .await?;

        Ok(models)
    }

    /// 查询设备的待执行指令
    pub async fn find_pending_by_device(
        &self,
        device_id: &str,
    ) -> anyhow::Result<Vec<CommandModel>> {
        let models = entities::Entity::find()
            .filter(entities::Column::DeviceId.eq(device_id))
            .filter(entities::Column::Status.eq("Pending"))
            .order_by_asc(entities::Column::CreatedAt)
            .all(&*self.db)
            .await?;

        Ok(models)
    }

    /// 删除指令
    pub async fn delete(&self, command_id: &str) -> anyhow::Result<()> {
        entities::Entity::delete_by_id(command_id.to_string())
            .exec(&*self.db)
            .await?;

        info!(command_id = %command_id, "Command deleted from database");
        Ok(())
    }

    /// 清理已完成的指令（保留最近 N 条）
    pub async fn cleanup_completed(&self, keep_last: u64) -> anyhow::Result<u64> {
        // 查找所有已完成的指令
        let completed_statuses = vec!["Success", "Failed", "Timeout", "Cancelled"];
        
        let mut total_deleted = 0u64;
        
        for status in completed_statuses {
            // 获取该状态的所有指令，按时间倒序
            let models = entities::Entity::find()
                .filter(entities::Column::Status.eq(status))
                .order_by_desc(entities::Column::CompletedAt)
                .all(&*self.db)
                .await?;

            // 删除超过保留数量的指令
            if models.len() > keep_last as usize {
                for model in models.iter().skip(keep_last as usize) {
                    entities::Entity::delete_by_id(model.id.clone())
                        .exec(&*self.db)
                        .await?;
                    total_deleted += 1;
                }
            }
        }

        if total_deleted > 0 {
            info!(deleted = total_deleted, "Cleaned up completed commands");
        }

        Ok(total_deleted)
    }

    /// 统计指令数量
    pub async fn count_by_device(&self, device_id: &str) -> anyhow::Result<u64> {
        let count = entities::Entity::find()
            .filter(entities::Column::DeviceId.eq(device_id))
            .count(&*self.db)
            .await?;

        Ok(count)
    }

    /// 统计各状态的指令数量
    pub async fn count_by_status(&self) -> anyhow::Result<std::collections::HashMap<String, u64>> {
        use sea_orm::sea_query::{Expr, Func};
        
        let results = entities::Entity::find()
            .select_only()
            .column(entities::Column::Status)
            .column_as(Expr::col(entities::Column::Id).count(), "count")
            .group_by(entities::Column::Status)
            .into_json()
            .all(&*self.db)
            .await?;

        let mut counts = std::collections::HashMap::new();
        for result in results {
            if let (Some(status), Some(count)) = (
                result.get("status").and_then(|v| v.as_str()),
                result.get("count").and_then(|v| v.as_u64()),
            ) {
                counts.insert(status.to_string(), count);
            }
        }

        Ok(counts)
    }
}
