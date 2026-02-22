use dashmap::DashMap;
use std::sync::Arc;
use tracing::debug;

/// 主题订阅管理器
#[derive(Clone)]
pub struct TopicMatcher {
    /// 订阅映射：topic_filter -> Vec<client_id>
    subscriptions: Arc<DashMap<String, Vec<String>>>,
}

impl Default for TopicMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl TopicMatcher {
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(DashMap::new()),
        }
    }

    /// 添加订阅
    pub fn subscribe(&self, client_id: String, topic_filter: String) {
        self.subscriptions
            .entry(topic_filter.clone())
            .or_default()
            .push(client_id.clone());

        debug!(
            client_id = %client_id,
            topic_filter = %topic_filter,
            "Client subscribed"
        );
    }

    /// 取消订阅
    pub fn unsubscribe(&self, client_id: &str, topic_filter: &str) {
        if let Some(mut clients) = self.subscriptions.get_mut(topic_filter) {
            clients.retain(|id| id != client_id);
            if clients.is_empty() {
                drop(clients);
                self.subscriptions.remove(topic_filter);
            }
        }

        debug!(
            client_id = %client_id,
            topic_filter = %topic_filter,
            "Client unsubscribed"
        );
    }

    /// 移除客户端的所有订阅
    pub fn remove_client(&self, client_id: &str) {
        self.subscriptions.retain(|_, clients| {
            clients.retain(|id| id != client_id);
            !clients.is_empty()
        });

        debug!(client_id = %client_id, "All subscriptions removed");
    }

    /// 查找匹配主题的所有客户端
    pub fn find_matching_clients(&self, topic: &str) -> Vec<String> {
        let mut clients = Vec::new();

        for entry in self.subscriptions.iter() {
            let (filter, client_ids) = entry.pair();
            if Self::matches(filter, topic) {
                clients.extend(client_ids.clone());
            }
        }

        // 去重
        clients.sort();
        clients.dedup();

        clients
    }

    /// 主题匹配算法
    ///
    /// MQTT 通配符规则：
    /// - `+` 匹配单个层级
    /// - `#` 匹配多个层级（只能在末尾）
    ///
    /// 示例：
    /// - `sensor/+/temperature` 匹配 `sensor/room1/temperature`
    /// - `sensor/#` 匹配 `sensor/room1/temperature` 和 `sensor/room1`
    pub fn matches(filter: &str, topic: &str) -> bool {
        // 快速路径：无通配符
        if !filter.contains('+') && !filter.contains('#') {
            return filter == topic;
        }

        let filter_parts: Vec<&str> = filter.split('/').collect();
        let topic_parts: Vec<&str> = topic.split('/').collect();

        Self::matches_parts(&filter_parts, &topic_parts)
    }

    fn matches_parts(filter: &[&str], topic: &[&str]) -> bool {
        match (filter.first(), topic.first()) {
            // 两者都为空，匹配成功
            (None, None) => true,

            // # 匹配所有剩余层级
            (Some(&"#"), _) => true,

            // + 匹配单个层级
            (Some(&"+"), Some(_)) => Self::matches_parts(&filter[1..], &topic[1..]),

            // 精确匹配当前层级
            (Some(f), Some(t)) if f == t => Self::matches_parts(&filter[1..], &topic[1..]),

            // 其他情况不匹配
            _ => false,
        }
    }

    /// 获取订阅数量
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// 获取客户端订阅的主题列表
    pub fn get_client_subscriptions(&self, client_id: &str) -> Vec<String> {
        self.subscriptions
            .iter()
            .filter(|entry| entry.value().contains(&client_id.to_string()))
            .map(|entry| entry.key().clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(TopicMatcher::matches(
            "sensor/temperature",
            "sensor/temperature"
        ));
        assert!(!TopicMatcher::matches(
            "sensor/temperature",
            "sensor/humidity"
        ));
    }

    #[test]
    fn test_single_level_wildcard() {
        // + 匹配单个层级
        assert!(TopicMatcher::matches(
            "sensor/+/temperature",
            "sensor/room1/temperature"
        ));
        assert!(TopicMatcher::matches(
            "sensor/+/temperature",
            "sensor/room2/temperature"
        ));
        assert!(!TopicMatcher::matches(
            "sensor/+/temperature",
            "sensor/room1/room2/temperature"
        ));

        // 多个 +
        assert!(TopicMatcher::matches(
            "+/+/temperature",
            "sensor/room1/temperature"
        ));
        assert!(TopicMatcher::matches(
            "sensor/+/+",
            "sensor/room1/temperature"
        ));
    }

    #[test]
    fn test_multi_level_wildcard() {
        // # 匹配多个层级
        assert!(TopicMatcher::matches("sensor/#", "sensor/temperature"));
        assert!(TopicMatcher::matches(
            "sensor/#",
            "sensor/room1/temperature"
        ));
        assert!(TopicMatcher::matches(
            "sensor/#",
            "sensor/room1/room2/temperature"
        ));
        assert!(!TopicMatcher::matches("sensor/#", "device/temperature"));

        // # 只能在末尾
        assert!(TopicMatcher::matches("#", "sensor/temperature"));
        assert!(TopicMatcher::matches("#", "anything/goes/here"));
    }

    #[test]
    fn test_combined_wildcards() {
        assert!(TopicMatcher::matches(
            "sensor/+/#",
            "sensor/room1/temperature"
        ));
        assert!(TopicMatcher::matches(
            "sensor/+/#",
            "sensor/room1/temperature/value"
        ));
        assert!(!TopicMatcher::matches("sensor/+/#", "sensor"));
    }

    #[test]
    fn test_topic_matcher() {
        let matcher = TopicMatcher::new();

        // 添加订阅
        matcher.subscribe("client1".to_string(), "sensor/+/temperature".to_string());
        matcher.subscribe("client2".to_string(), "sensor/#".to_string());
        matcher.subscribe(
            "client3".to_string(),
            "sensor/room1/temperature".to_string(),
        );

        // 查找匹配的客户端
        let clients = matcher.find_matching_clients("sensor/room1/temperature");
        assert_eq!(clients.len(), 3);
        assert!(clients.contains(&"client1".to_string()));
        assert!(clients.contains(&"client2".to_string()));
        assert!(clients.contains(&"client3".to_string()));

        // 取消订阅
        matcher.unsubscribe("client1", "sensor/+/temperature");
        let clients = matcher.find_matching_clients("sensor/room1/temperature");
        assert_eq!(clients.len(), 2);

        // 移除客户端
        matcher.remove_client("client2");
        let clients = matcher.find_matching_clients("sensor/room1/temperature");
        assert_eq!(clients.len(), 1);
    }
}
