use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, DatabaseConnection, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// 会话数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub client_id: String,
    pub clean_session: bool,
    pub subscriptions: Vec<Subscription>,
    pub will: Option<WillMessage>,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 订阅信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub topic_filter: String,
    pub qos: u8,
}

/// Will 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WillMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub retained: bool,
}

/// 会话存储
pub struct SessionStore {
    db: Arc<DatabaseConnection>,
}

impl SessionStore {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 保存会话
    pub async fn save(&self, session: &SessionData) -> Result<(), DbErr> {
        use crate::db::mqtt_session;

        let subscriptions_json = serde_json::to_string(&session.subscriptions)
            .unwrap_or_else(|_| "[]".to_string());

        let (will_topic, will_payload, will_qos, will_retained) = if let Some(will) = &session.will {
            (
                Some(will.topic.clone()),
                Some(will.payload.clone()),
                Some(will.qos as i16),
                Some(will.retained),
            )
        } else {
            (None, None, None, None)
        };

        let model = mqtt_session::ActiveModel {
            client_id: Set(session.client_id.clone()),
            clean_session: Set(session.clean_session),
            subscriptions: Set(Some(subscriptions_json)),
            will_topic: Set(will_topic),
            will_payload: Set(will_payload),
            will_qos: Set(will_qos),
            will_retained: Set(will_retained),
            created_at: Set(session.created_at),
            last_seen: Set(session.last_seen),
            expires_at: Set(session.expires_at),
        };

        mqtt_session::Entity::insert(model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(mqtt_session::Column::ClientId)
                    .update_columns([
                        mqtt_session::Column::CleanSession,
                        mqtt_session::Column::Subscriptions,
                        mqtt_session::Column::WillTopic,
                        mqtt_session::Column::WillPayload,
                        mqtt_session::Column::WillQos,
                        mqtt_session::Column::WillRetained,
                        mqtt_session::Column::LastSeen,
                        mqtt_session::Column::ExpiresAt,
                    ])
                    .to_owned(),
            )
            .exec(&*self.db)
            .await?;

        info!(client_id = %session.client_id, "Session saved to database");
        Ok(())
    }

    /// 加载会话
    pub async fn load(&self, client_id: &str) -> Result<Option<SessionData>, DbErr> {
        use crate::db::mqtt_session;

        let model = mqtt_session::Entity::find_by_id(client_id.to_string())
            .one(&*self.db)
            .await?;

        if let Some(model) = model {
            let subscriptions: Vec<Subscription> = model
                .subscriptions
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            let will = if let Some(topic) = model.will_topic {
                Some(WillMessage {
                    topic,
                    payload: model.will_payload.unwrap_or_default(),
                    qos: model.will_qos.unwrap_or(0) as u8,
                    retained: model.will_retained.unwrap_or(false),
                })
            } else {
                None
            };

            let session = SessionData {
                client_id: model.client_id,
                clean_session: model.clean_session,
                subscriptions,
                will,
                created_at: model.created_at,
                last_seen: model.last_seen,
                expires_at: model.expires_at,
            };

            debug!(client_id = %client_id, "Session loaded from database");
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    /// 删除会话
    pub async fn delete(&self, client_id: &str) -> Result<(), DbErr> {
        use crate::db::mqtt_session;

        mqtt_session::Entity::delete_by_id(client_id.to_string())
            .exec(&*self.db)
            .await?;

        info!(client_id = %client_id, "Session deleted from database");
        Ok(())
    }

    /// 更新最后活跃时间
    pub async fn update_last_seen(&self, client_id: &str) -> Result<(), DbErr> {
        use crate::db::mqtt_session;

        mqtt_session::Entity::update_many()
            .col_expr(
                mqtt_session::Column::LastSeen,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(mqtt_session::Column::ClientId.eq(client_id))
            .exec(&*self.db)
            .await?;

        Ok(())
    }

    /// 清理过期会话
    pub async fn cleanup_expired(&self) -> Result<u64, DbErr> {
        use crate::db::mqtt_session;

        let now = Utc::now();
        let result = mqtt_session::Entity::delete_many()
            .filter(mqtt_session::Column::ExpiresAt.lt(now))
            .exec(&*self.db)
            .await?;

        if result.rows_affected > 0 {
            info!(count = result.rows_affected, "Expired sessions cleaned up");
        }

        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_data_creation() {
        let session = SessionData {
            client_id: "test_client".to_string(),
            clean_session: false,
            subscriptions: vec![Subscription {
                topic_filter: "test/topic".to_string(),
                qos: 1,
            }],
            will: None,
            created_at: Utc::now(),
            last_seen: Utc::now(),
            expires_at: None,
        };

        assert_eq!(session.client_id, "test_client");
        assert!(!session.clean_session);
        assert_eq!(session.subscriptions.len(), 1);
    }

    #[test]
    fn test_will_message() {
        let will = WillMessage {
            topic: "client/status".to_string(),
            payload: b"offline".to_vec(),
            qos: 1,
            retained: true,
        };

        assert_eq!(will.topic, "client/status");
        assert_eq!(will.payload, b"offline");
        assert_eq!(will.qos, 1);
        assert!(will.retained);
    }
}
