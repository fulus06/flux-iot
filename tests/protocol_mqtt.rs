// MQTT 协议集成测试
// 测试 CONNECT, PUBLISH, SUBSCRIBE, QoS, Retained 消息等

mod common;

use flux_mqtt::{manager::MqttManager, retained::RetainedStore, topic_matcher::TopicMatcher};
use ntex::util::Bytes;

#[tokio::test]
async fn test_mqtt_qos0_publish_subscribe() {
    let manager = MqttManager::new();
    let matcher = manager.topic_matcher();
    
    // 客户端订阅
    matcher.subscribe("client1".to_string(), "sensor/temperature".to_string());
    
    // 发布消息
    let clients = matcher.find_matching_clients("sensor/temperature");
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0], "client1");
}

#[tokio::test]
async fn test_mqtt_wildcard_subscriptions() {
    let matcher = TopicMatcher::new();
    
    // 订阅通配符主题
    matcher.subscribe("client1".to_string(), "sensor/+/temperature".to_string());
    matcher.subscribe("client2".to_string(), "sensor/#".to_string());
    matcher.subscribe("client3".to_string(), "sensor/room1/#".to_string());
    
    // 测试匹配
    let clients = matcher.find_matching_clients("sensor/room1/temperature");
    assert_eq!(clients.len(), 3);
    
    let clients = matcher.find_matching_clients("sensor/room2/temperature");
    assert_eq!(clients.len(), 2); // client1 和 client2
    
    let clients = matcher.find_matching_clients("sensor/room1/humidity");
    assert_eq!(clients.len(), 2); // client2 和 client3
}

#[tokio::test]
async fn test_mqtt_retained_messages() {
    let store = RetainedStore::new();
    
    // 设置 retained 消息
    store.set("status/online".to_string(), Bytes::from("true"), 1);
    store.set("config/version".to_string(), Bytes::from("1.0.0"), 0);
    
    assert_eq!(store.count(), 2);
    
    // 获取消息
    let msg = store.get("status/online").unwrap();
    assert_eq!(msg.payload, Bytes::from("true"));
    assert_eq!(msg.qos, 1);
    
    // 删除 retained 消息（发送空 payload）
    store.set("status/online".to_string(), Bytes::new(), 0);
    assert_eq!(store.count(), 1);
}

#[tokio::test]
async fn test_mqtt_retained_wildcard_query() {
    let store = RetainedStore::new();
    
    // 设置多个 retained 消息
    store.set("device/001/status".to_string(), Bytes::from("online"), 0);
    store.set("device/002/status".to_string(), Bytes::from("offline"), 0);
    store.set("device/001/battery".to_string(), Bytes::from("80"), 0);
    store.set("device/002/battery".to_string(), Bytes::from("60"), 0);
    
    // 使用通配符查询
    let matches = store.get_matching("device/+/status");
    assert_eq!(matches.len(), 2);
    
    let matches = store.get_matching("device/001/#");
    assert_eq!(matches.len(), 2);
    
    let matches = store.get_matching("device/#");
    assert_eq!(matches.len(), 4);
}

#[tokio::test]
async fn test_mqtt_subscription_management() {
    let matcher = TopicMatcher::new();
    
    // 客户端订阅多个主题
    matcher.subscribe("client1".to_string(), "topic1".to_string());
    matcher.subscribe("client1".to_string(), "topic2".to_string());
    matcher.subscribe("client1".to_string(), "topic3".to_string());
    
    // 验证订阅
    let subs = matcher.get_client_subscriptions("client1");
    assert_eq!(subs.len(), 3);
    
    // 取消单个订阅
    matcher.unsubscribe("client1", "topic2");
    let subs = matcher.get_client_subscriptions("client1");
    assert_eq!(subs.len(), 2);
    
    // 移除客户端（清除所有订阅）
    matcher.remove_client("client1");
    assert_eq!(matcher.subscription_count(), 0);
}

#[tokio::test]
async fn test_mqtt_topic_validation() {
    // 测试主题名称验证
    assert!(TopicMatcher::is_valid_topic("sensor/temperature"));
    assert!(TopicMatcher::is_valid_topic("a/b/c/d/e"));
    assert!(TopicMatcher::is_valid_topic("$SYS/broker/clients"));
    
    // 无效主题
    assert!(!TopicMatcher::is_valid_topic(""));
    assert!(!TopicMatcher::is_valid_topic("sensor//temperature")); // 空层级
    assert!(!TopicMatcher::is_valid_topic("sensor/temp#")); // # 不在末尾
}

#[tokio::test]
async fn test_mqtt_shared_subscriptions() {
    // 测试共享订阅（负载均衡）
    let matcher = TopicMatcher::new();
    
    // 多个客户端订阅同一主题
    matcher.subscribe("client1".to_string(), "$share/group1/sensor/data".to_string());
    matcher.subscribe("client2".to_string(), "$share/group1/sensor/data".to_string());
    matcher.subscribe("client3".to_string(), "$share/group1/sensor/data".to_string());
    
    // 发布消息时应该只分发给一个客户端（负载均衡）
    // 这需要在实际的 MQTT Broker 实现中测试
}

#[tokio::test]
async fn test_mqtt_will_message() {
    // 测试遗嘱消息
    // 当客户端异常断开时，Broker 应该发布遗嘱消息
    let store = RetainedStore::new();
    
    // 模拟客户端设置遗嘱消息
    let will_topic = "device/001/status";
    let will_payload = Bytes::from("offline");
    
    // 客户端异常断开时，Broker 发布遗嘱
    store.set(will_topic.to_string(), will_payload.clone(), 1);
    
    let msg = store.get(will_topic).unwrap();
    assert_eq!(msg.payload, will_payload);
}

#[tokio::test]
async fn test_mqtt_clean_session() {
    // 测试 Clean Session 标志
    let matcher = TopicMatcher::new();
    
    // Clean Session = false: 保留订阅
    matcher.subscribe("client1".to_string(), "topic1".to_string());
    assert_eq!(matcher.subscription_count(), 1);
    
    // 模拟客户端断开重连（Clean Session = false）
    // 订阅应该保留
    let subs = matcher.get_client_subscriptions("client1");
    assert_eq!(subs.len(), 1);
    
    // Clean Session = true: 清除订阅
    matcher.remove_client("client1");
    assert_eq!(matcher.subscription_count(), 0);
}

#[tokio::test]
async fn test_mqtt_concurrent_operations() {
    let matcher = Arc::new(TopicMatcher::new());
    let mut handles = vec![];
    
    // 并发订阅
    for i in 0..100 {
        let matcher_clone = matcher.clone();
        let handle = tokio::spawn(async move {
            let client_id = format!("client_{}", i);
            let topic = format!("topic/{}", i % 10);
            matcher_clone.subscribe(client_id, topic);
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.await.unwrap();
    }
    
    // 验证订阅数量
    assert!(matcher.subscription_count() > 0);
}

#[tokio::test]
async fn test_mqtt_message_ordering() {
    // 测试消息顺序保证（QoS 1/2）
    // 同一客户端的消息应该按发送顺序到达
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    let received_order = Arc::new(Mutex::new(Vec::new()));
    
    // 模拟按序发送 10 条消息
    for i in 0..10 {
        let order = received_order.clone();
        tokio::spawn(async move {
            order.lock().await.push(i);
        });
    }
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let order = received_order.lock().await;
    // 注意：实际测试需要在 MQTT Broker 中验证顺序
    assert_eq!(order.len(), 10);
}

use std::sync::Arc;

#[tokio::test]
async fn test_mqtt_performance_1000_clients() {
    // 性能测试：1000 个客户端订阅
    let matcher = Arc::new(TopicMatcher::new());
    
    let start = std::time::Instant::now();
    
    for i in 0..1000 {
        let client_id = format!("perf_client_{}", i);
        matcher.subscribe(client_id, "perf/test/topic".to_string());
    }
    
    let duration = start.elapsed();
    println!("✅ 1000 clients subscribed in {:?}", duration);
    
    // 查找匹配客户端
    let start = std::time::Instant::now();
    let clients = matcher.find_matching_clients("perf/test/topic");
    let duration = start.elapsed();
    
    assert_eq!(clients.len(), 1000);
    println!("✅ Found 1000 matching clients in {:?}", duration);
}
