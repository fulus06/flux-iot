use flux_mqtt::{manager::MqttManager, retained::RetainedStore, topic_matcher::TopicMatcher};
use ntex::util::Bytes;

#[tokio::test]
async fn test_mqtt_manager_creation() {
    let manager = MqttManager::new();
    assert_eq!(manager.topic_matcher().subscription_count(), 0);
    assert_eq!(manager.retained_store().count(), 0);
}

#[tokio::test]
async fn test_retained_message_workflow() {
    let store = RetainedStore::new();

    // 设置 retained 消息
    store.set("sensor/temperature".to_string(), Bytes::from("25.5"), 1);

    // 验证消息已保存
    assert_eq!(store.count(), 1);

    // 获取消息
    let msg = store.get("sensor/temperature").unwrap();
    assert_eq!(msg.payload, Bytes::from("25.5"));
    assert_eq!(msg.qos, 1);

    // 删除消息（空 payload）
    store.set("sensor/temperature".to_string(), Bytes::new(), 0);
    assert_eq!(store.count(), 0);
}

#[tokio::test]
async fn test_topic_wildcard_matching() {
    let matcher = TopicMatcher::new();

    // 添加订阅
    matcher.subscribe("client1".to_string(), "sensor/+/temperature".to_string());
    matcher.subscribe("client2".to_string(), "sensor/#".to_string());
    matcher.subscribe(
        "client3".to_string(),
        "sensor/room1/temperature".to_string(),
    );

    // 测试匹配
    let clients = matcher.find_matching_clients("sensor/room1/temperature");
    assert_eq!(clients.len(), 3);
    assert!(clients.contains(&"client1".to_string()));
    assert!(clients.contains(&"client2".to_string()));
    assert!(clients.contains(&"client3".to_string()));

    // 测试单级通配符
    let clients = matcher.find_matching_clients("sensor/room2/temperature");
    assert_eq!(clients.len(), 2); // client1 和 client2

    // 测试多级通配符
    let clients = matcher.find_matching_clients("sensor/room1/humidity");
    assert_eq!(clients.len(), 1); // 只有 client2
}

#[tokio::test]
async fn test_retained_with_wildcards() {
    let store = RetainedStore::new();

    // 设置多个 retained 消息
    store.set("sensor/temp/room1".to_string(), Bytes::from("20"), 0);
    store.set("sensor/temp/room2".to_string(), Bytes::from("22"), 0);
    store.set("sensor/humidity/room1".to_string(), Bytes::from("60"), 0);

    // 使用单级通配符查询
    let matches = store.get_matching("sensor/temp/+");
    assert_eq!(matches.len(), 2);

    // 使用多级通配符查询
    let matches = store.get_matching("sensor/#");
    assert_eq!(matches.len(), 3);

    // 精确匹配
    let matches = store.get_matching("sensor/temp/room1");
    assert_eq!(matches.len(), 1);
}

#[tokio::test]
async fn test_subscription_management() {
    let matcher = TopicMatcher::new();

    // 订阅
    matcher.subscribe("client1".to_string(), "test/topic".to_string());
    assert_eq!(matcher.subscription_count(), 1);

    // 获取客户端订阅
    let subs = matcher.get_client_subscriptions("client1");
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0], "test/topic");

    // 取消订阅
    matcher.unsubscribe("client1", "test/topic");
    assert_eq!(matcher.subscription_count(), 0);
}

#[tokio::test]
async fn test_client_removal() {
    let matcher = TopicMatcher::new();

    // 客户端订阅多个主题
    matcher.subscribe("client1".to_string(), "topic1".to_string());
    matcher.subscribe("client1".to_string(), "topic2".to_string());
    matcher.subscribe("client2".to_string(), "topic1".to_string());

    assert_eq!(matcher.subscription_count(), 2); // topic1 和 topic2

    // 移除客户端
    matcher.remove_client("client1");

    // topic1 还有 client2，topic2 被删除
    assert_eq!(matcher.subscription_count(), 1);

    let clients = matcher.find_matching_clients("topic1");
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0], "client2");
}

#[tokio::test]
async fn test_complex_wildcard_patterns() {
    let _matcher = TopicMatcher::new();

    // 测试复杂通配符模式
    assert!(TopicMatcher::matches(
        "sensor/+/#",
        "sensor/room1/temperature"
    ));
    assert!(TopicMatcher::matches(
        "sensor/+/#",
        "sensor/room1/temp/value"
    ));
    assert!(!TopicMatcher::matches("sensor/+/#", "sensor"));

    // 测试 # 在开头
    assert!(TopicMatcher::matches("#", "any/topic/here"));
    assert!(TopicMatcher::matches("#", "single"));

    // 测试多个 +
    assert!(TopicMatcher::matches("+/+/+", "a/b/c"));
    assert!(!TopicMatcher::matches("+/+/+", "a/b"));
}
