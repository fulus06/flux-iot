use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, DatabaseConnection, QueryOrder, QuerySelect, Set};
use std::sync::Arc;
use tracing::{debug, info};

/// 离线消息
#[derive(Debug, Clone)]
pub struct OfflineMessage {
    pub id: Option<i64>,
    pub client_id: String,
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub retained: bool,
    pub created_at: DateTime<Utc>,
}

/// 离线消息存储
pub struct OfflineMessageStore {
    db: Arc<DatabaseConnection>,
    max_messages_per_client: usize,
}

impl OfflineMessageStore {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            max_messages_per_client: 1000, // 默认每个客户端最多保存 1000 条离线消息
        }
    }

    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages_per_client = max;
        self
    }

    /// 保存离线消息
    pub async fn save(&self, message: &OfflineMessage) -> Result<(), DbErr> {
        use crate::db::mqtt_offline_message;

        // 检查该客户端的离线消息数量
        let count = mqtt_offline_message::Entity::find()
            .filter(mqtt_offline_message::Column::ClientId.eq(&message.client_id))
            .count(&*self.db)
            .await?;

        // 如果超过限制，删除最旧的消息
        if count >= self.max_messages_per_client as u64 {
            let to_delete = count - self.max_messages_per_client as u64 + 1;
            
            // 获取最旧的消息 ID
            let old_messages = mqtt_offline_message::Entity::find()
                .filter(mqtt_offline_message::Column::ClientId.eq(&message.client_id))
                .order_by_asc(mqtt_offline_message::Column::CreatedAt)
                .limit(to_delete)
                .all(&*self.db)
                .await?;

            for old_msg in old_messages {
                mqtt_offline_message::Entity::delete_by_id(old_msg.id)
                    .exec(&*self.db)
                    .await?;
            }

            debug!(
                client_id = %message.client_id,
                deleted = to_delete,
                "Deleted old offline messages"
            );
        }

        // 保存新消息
        let model = mqtt_offline_message::ActiveModel {
            id: Set(None),
            client_id: Set(message.client_id.clone()),
            topic: Set(message.topic.clone()),
            payload: Set(message.payload.clone()),
            qos: Set(message.qos as i16),
            retained: Set(message.retained),
            created_at: Set(message.created_at),
        };

        mqtt_offline_message::Entity::insert(model)
            .exec(&*self.db)
            .await?;

        debug!(
            client_id = %message.client_id,
            topic = %message.topic,
            "Offline message saved"
        );

        Ok(())
    }

    /// 获取客户端的所有离线消息
    pub async fn get_messages(&self, client_id: &str) -> Result<Vec<OfflineMessage>, DbErr> {
        use crate::db::mqtt_offline_message;

        let models = mqtt_offline_message::Entity::find()
            .filter(mqtt_offline_message::Column::ClientId.eq(client_id))
            .order_by_asc(mqtt_offline_message::Column::CreatedAt)
            .all(&*self.db)
            .await?;

        let messages: Vec<OfflineMessage> = models
            .into_iter()
            .map(|m| OfflineMessage {
                id: Some(m.id),
                client_id: m.client_id,
                topic: m.topic,
                payload: m.payload,
                qos: m.qos as u8,
                retained: m.retained,
                created_at: m.created_at,
            })
            .collect();

        debug!(
            client_id = %client_id,
            count = messages.len(),
            "Offline messages retrieved"
        );

        Ok(messages)
    }

    /// 删除客户端的所有离线消息
    pub async fn delete_messages(&self, client_id: &str) -> Result<u64, DbErr> {
        use crate::db::mqtt_offline_message;

        let result = mqtt_offline_message::Entity::delete_many()
            .filter(mqtt_offline_message::Column::ClientId.eq(client_id))
            .exec(&*self.db)
            .await?;

        info!(
            client_id = %client_id,
            count = result.rows_affected,
            "Offline messages deleted"
        );

        Ok(result.rows_affected)
    }

    /// 删除单条离线消息
    pub async fn delete_message(&self, message_id: i64) -> Result<(), DbErr> {
        use crate::db::mqtt_offline_message;

        mqtt_offline_message::Entity::delete_by_id(message_id)
            .exec(&*self.db)
            .await?;

        Ok(())
    }

    /// 清理过期的离线消息（超过指定天数）
    pub async fn cleanup_old_messages(&self, days: i64) -> Result<u64, DbErr> {
        use crate::db::mqtt_offline_message;
        use chrono::Duration;

        let cutoff = Utc::now() - Duration::days(days);
        let result = mqtt_offline_message::Entity::delete_many()
            .filter(mqtt_offline_message::Column::CreatedAt.lt(cutoff))
            .exec(&*self.db)
            .await?;

        if result.rows_affected > 0 {
            info!(
                count = result.rows_affected,
                days = days,
                "Old offline messages cleaned up"
            );
        }

        Ok(result.rows_affected)
    }

    /// 获取离线消息统计
    pub async fn get_stats(&self) -> Result<OfflineMessageStats, DbErr> {
        use crate::db::mqtt_offline_message;

        let total = mqtt_offline_message::Entity::find()
            .count(&*self.db)
            .await?;

        // 获取每个客户端的消息数量（这里简化处理，实际可能需要更复杂的查询）
        Ok(OfflineMessageStats {
            total_messages: total,
            max_per_client: self.max_messages_per_client,
        })
    }
}

/// 离线消息统计
#[derive(Debug, Clone)]
pub struct OfflineMessageStats {
    pub total_messages: u64,
    pub max_per_client: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offline_message_creation() {
        let message = OfflineMessage {
            id: None,
            client_id: "test_client".to_string(),
            topic: "test/topic".to_string(),
            payload: b"test payload".to_vec(),
            qos: 1,
            retained: false,
            created_at: Utc::now(),
        };

        assert_eq!(message.client_id, "test_client");
        assert_eq!(message.topic, "test/topic");
        assert_eq!(message.qos, 1);
    }
}
