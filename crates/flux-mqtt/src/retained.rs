use dashmap::DashMap;
use ntex::util::Bytes;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info};

/// Retained 消息
#[derive(Clone, Debug)]
pub struct RetainedMessage {
    pub topic: String,
    pub payload: Bytes,
    pub qos: u8,
    pub timestamp: SystemTime,
}

/// Retained 消息存储
#[derive(Clone)]
pub struct RetainedStore {
    messages: Arc<DashMap<String, RetainedMessage>>,
}

impl Default for RetainedStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RetainedStore {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(DashMap::new()),
        }
    }

    /// 设置 retained 消息
    pub fn set(&self, topic: String, payload: Bytes, qos: u8) {
        if payload.is_empty() {
            // 空 payload 表示删除 retained 消息
            self.messages.remove(&topic);
            debug!(topic = %topic, "Retained message removed");
        } else {
            let msg = RetainedMessage {
                topic: topic.clone(),
                payload,
                qos,
                timestamp: SystemTime::now(),
            };
            self.messages.insert(topic.clone(), msg);
            info!(topic = %topic, qos = qos, "Retained message stored");
        }
    }

    /// 获取 retained 消息
    pub fn get(&self, topic: &str) -> Option<RetainedMessage> {
        self.messages.get(topic).map(|entry| entry.value().clone())
    }

    /// 获取匹配主题的所有 retained 消息
    pub fn get_matching(&self, topic_filter: &str) -> Vec<RetainedMessage> {
        self.messages
            .iter()
            .filter(|entry| Self::topic_matches(topic_filter, entry.key()))
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 主题匹配（支持通配符）
    fn topic_matches(filter: &str, topic: &str) -> bool {
        // 如果没有通配符，直接比较
        if !filter.contains('+') && !filter.contains('#') {
            return filter == topic;
        }

        let filter_parts: Vec<&str> = filter.split('/').collect();
        let topic_parts: Vec<&str> = topic.split('/').collect();

        Self::matches_parts(&filter_parts, &topic_parts)
    }

    fn matches_parts(filter: &[&str], topic: &[&str]) -> bool {
        match (filter.first(), topic.first()) {
            (None, None) => true,
            (Some(&"#"), _) => true, // # 匹配所有剩余层级
            (Some(&"+"), Some(_)) => {
                // + 匹配单个层级
                Self::matches_parts(&filter[1..], &topic[1..])
            }
            (Some(f), Some(t)) if f == t => Self::matches_parts(&filter[1..], &topic[1..]),
            _ => false,
        }
    }

    /// 清空所有 retained 消息
    pub fn clear(&self) {
        self.messages.clear();
        info!("All retained messages cleared");
    }

    /// 获取 retained 消息数量
    pub fn count(&self) -> usize {
        self.messages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retained_store() {
        let store = RetainedStore::new();

        // 设置 retained 消息
        store.set("sensor/temperature".to_string(), Bytes::from("25.5"), 1);

        // 获取消息
        let msg = store.get("sensor/temperature").unwrap();
        assert_eq!(msg.payload, Bytes::from("25.5"));
        assert_eq!(msg.qos, 1);

        // 删除消息（空 payload）
        store.set("sensor/temperature".to_string(), Bytes::new(), 0);
        assert!(store.get("sensor/temperature").is_none());
    }

    #[test]
    fn test_topic_matching() {
        let store = RetainedStore::new();

        store.set("sensor/temp/room1".to_string(), Bytes::from("20"), 0);
        store.set("sensor/temp/room2".to_string(), Bytes::from("22"), 0);
        store.set("sensor/humidity/room1".to_string(), Bytes::from("60"), 0);

        // 单级通配符
        let matches = store.get_matching("sensor/temp/+");
        assert_eq!(matches.len(), 2);

        // 多级通配符
        let matches = store.get_matching("sensor/#");
        assert_eq!(matches.len(), 3);

        // 精确匹配
        let matches = store.get_matching("sensor/temp/room1");
        assert_eq!(matches.len(), 1);
    }
}
