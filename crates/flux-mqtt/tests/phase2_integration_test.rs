use flux_mqtt::{
    acl::{AclAction, AclPermission, AclRule, MqttAcl},
    manager::MqttManager,
    metrics::MqttMetrics,
};

#[tokio::test]
async fn test_mqtt_manager_with_acl() {
    // 创建 ACL 规则
    let rules = vec![
        AclRule {
            client_id: Some("sensor_*".to_string()),
            username: None,
            topic_pattern: "sensor/+/data".to_string(),
            action: AclAction::Publish,
            permission: AclPermission::Allow,
            priority: 10,
        },
        AclRule {
            client_id: None,
            username: Some("admin".to_string()),
            topic_pattern: "#".to_string(),
            action: AclAction::Both,
            permission: AclPermission::Allow,
            priority: 100,
        },
    ];

    let acl = MqttAcl::new(rules);
    let manager = MqttManager::new().with_acl(acl);

    // 验证 ACL 已设置
    assert!(manager.acl().is_some());

    // 测试权限检查
    let acl = manager.acl().unwrap();
    assert!(acl.check_publish("sensor_001", None, "sensor/room1/data"));
    assert!(!acl.check_publish("sensor_001", None, "admin/config"));
    assert!(acl.check_publish("any_client", Some("admin"), "admin/config"));
}

#[tokio::test]
async fn test_mqtt_manager_metrics() {
    let manager = MqttManager::new();
    let metrics = manager.metrics();

    // 初始状态
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.connections_current, 0);
    assert_eq!(snapshot.messages_published, 0);

    // 模拟连接
    metrics.record_connection();
    metrics.record_connection();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.connections_current, 2);
    assert_eq!(snapshot.connections_total, 2);
    assert_eq!(snapshot.connections_peak, 2);

    // 模拟消息
    metrics.record_message_published(100, 1);
    metrics.record_message_received(50, 0);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.messages_published, 1);
    assert_eq!(snapshot.messages_received, 1);
    assert_eq!(snapshot.bytes_sent, 100);
    assert_eq!(snapshot.bytes_received, 50);
    assert_eq!(snapshot.qos0_messages, 1);
    assert_eq!(snapshot.qos1_messages, 1);

    // 模拟断开连接
    metrics.record_disconnection();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.connections_current, 1);
}

#[tokio::test]
async fn test_acl_wildcard_patterns() {
    let rules = vec![
        AclRule {
            client_id: Some("device_*".to_string()),
            username: None,
            topic_pattern: "device/+/status".to_string(),
            action: AclAction::Publish,
            permission: AclPermission::Allow,
            priority: 10,
        },
        AclRule {
            client_id: Some("*".to_string()),
            username: None,
            topic_pattern: "public/#".to_string(),
            action: AclAction::Subscribe,
            permission: AclPermission::Allow,
            priority: 5,
        },
    ];

    let acl = MqttAcl::new(rules);

    // 测试客户端 ID 通配符
    assert!(acl.check_publish("device_001", None, "device/sensor1/status"));
    assert!(acl.check_publish("device_abc", None, "device/sensor2/status"));
    assert!(!acl.check_publish("other_client", None, "device/sensor1/status"));

    // 测试主题通配符
    assert!(acl.check_subscribe("any_client", None, "public/news"));
    assert!(acl.check_subscribe("any_client", None, "public/weather/today"));
    assert!(!acl.check_subscribe("any_client", None, "private/data"));
}

#[tokio::test]
async fn test_acl_priority_ordering() {
    let rules = vec![
        // 低优先级：拒绝所有
        AclRule {
            client_id: Some("*".to_string()),
            username: None,
            topic_pattern: "#".to_string(),
            action: AclAction::Both,
            permission: AclPermission::Deny,
            priority: 0,
        },
        // 高优先级：允许管理员
        AclRule {
            client_id: Some("admin_*".to_string()),
            username: None,
            topic_pattern: "#".to_string(),
            action: AclAction::Both,
            permission: AclPermission::Allow,
            priority: 100,
        },
    ];

    let acl = MqttAcl::new(rules);

    // 管理员应该被允许（高优先级规则）
    assert!(acl.check_publish("admin_001", None, "any/topic"));
    assert!(acl.check_subscribe("admin_002", None, "any/topic"));

    // 普通客户端应该被拒绝（低优先级规则）
    assert!(!acl.check_publish("user_001", None, "any/topic"));
    assert!(!acl.check_subscribe("user_002", None, "any/topic"));
}

#[tokio::test]
async fn test_metrics_prometheus_export() {
    let metrics = MqttMetrics::new();

    // 记录一些指标
    metrics.record_connection();
    metrics.record_message_published(100, 1);
    metrics.record_retained_message_stored();

    // 导出 Prometheus 格式
    let prometheus = metrics.export_prometheus();

    // 验证包含关键指标
    assert!(prometheus.contains("mqtt_connections_current 1"));
    assert!(prometheus.contains("mqtt_messages_published_total 1"));
    assert!(prometheus.contains("mqtt_bytes_sent_total 100"));
    assert!(prometheus.contains("mqtt_retained_messages 1"));
    assert!(prometheus.contains("mqtt_qos1_messages_total 1"));

    // 验证格式正确
    assert!(prometheus.contains("# HELP"));
    assert!(prometheus.contains("# TYPE"));
}

#[tokio::test]
async fn test_subscription_metrics() {
    let manager = MqttManager::new();

    // 模拟订阅
    manager.metrics().record_subscription();
    manager.metrics().record_subscription();
    manager.metrics().record_subscription();

    let snapshot = manager.metrics().snapshot();
    assert_eq!(snapshot.subscriptions_current, 3);

    // 模拟取消订阅
    manager.metrics().record_unsubscription();

    let snapshot = manager.metrics().snapshot();
    assert_eq!(snapshot.subscriptions_current, 2);
}

#[tokio::test]
async fn test_retained_messages_metrics() {
    let manager = MqttManager::new();

    // 初始状态
    let snapshot = manager.metrics().snapshot();
    assert_eq!(snapshot.retained_messages, 0);

    // 存储 retained 消息
    manager.metrics().record_retained_message_stored();
    manager.metrics().record_retained_message_stored();

    let snapshot = manager.metrics().snapshot();
    assert_eq!(snapshot.retained_messages, 2);

    // 删除 retained 消息
    manager.metrics().record_retained_message_removed();

    let snapshot = manager.metrics().snapshot();
    assert_eq!(snapshot.retained_messages, 1);
}
