use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub topic: String,
    pub payload: serde_json::Value,
    pub timestamp: i64,
}

impl Message {
    pub fn new(topic: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            topic: topic.into(),
            payload,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}
