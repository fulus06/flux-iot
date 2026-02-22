use flux_core::bus::EventBus;
use flux_core::traits::auth::Authenticator;
use flux_mqtt::{
    acl::{AclAction, AclPermission, AclRule, MqttAcl},
    manager::MqttManager,
    start_broker,
};
use std::sync::Arc;
use tracing_subscriber;

/// 简单的认证器实现
struct SimpleAuthenticator;

#[async_trait::async_trait]
impl Authenticator for SimpleAuthenticator {
    async fn authenticate(
        &self,
        client_id: &str,
        username: Option<&str>,
        _password: Option<&[u8]>,
    ) -> anyhow::Result<bool> {
        tracing::info!(
            client_id = %client_id,
            username = ?username,
            "Client authentication"
        );
        Ok(true)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info,flux_mqtt=debug")
        .init();

    tracing::info!("Starting MQTT Broker with ACL Example");

    // 创建 EventBus
    let event_bus = Arc::new(EventBus::new(1000));

    // 创建认证器
    let authenticator = Arc::new(SimpleAuthenticator);

    // 创建 ACL 规则
    let rules = vec![
        // 传感器设备只能发布到 sensor/+/data
        AclRule {
            client_id: Some("sensor_*".to_string()),
            username: None,
            topic_pattern: "sensor/+/data".to_string(),
            action: AclAction::Publish,
            permission: AclPermission::Allow,
            priority: 10,
        },
        // 所有客户端可以订阅 public/#
        AclRule {
            client_id: Some("*".to_string()),
            username: None,
            topic_pattern: "public/#".to_string(),
            action: AclAction::Subscribe,
            permission: AclPermission::Allow,
            priority: 5,
        },
        // 管理员拥有所有权限
        AclRule {
            client_id: None,
            username: Some("admin".to_string()),
            topic_pattern: "#".to_string(),
            action: AclAction::Both,
            permission: AclPermission::Allow,
            priority: 100,
        },
        // 默认拒绝所有其他操作
        AclRule {
            client_id: Some("*".to_string()),
            username: None,
            topic_pattern: "#".to_string(),
            action: AclAction::Both,
            permission: AclPermission::Deny,
            priority: 0,
        },
    ];

    tracing::info!("ACL rules configured:");
    tracing::info!("  - sensor_* can publish to sensor/+/data");
    tracing::info!("  - * can subscribe to public/#");
    tracing::info!("  - admin has full access");
    tracing::info!("  - all other operations are denied");

    // 创建带 ACL 的管理器
    let acl = MqttAcl::new(rules);
    let _manager = MqttManager::new().with_acl(acl);

    // 启动 MQTT broker
    start_broker(event_bus.clone(), authenticator);

    // 订阅 EventBus 消息
    let mut rx = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            tracing::info!(
                topic = %msg.topic,
                payload = ?msg.payload,
                "Received message from EventBus"
            );
        }
    });

    // 保持运行
    tracing::info!("MQTT Broker with ACL is running on port 1883");
    tracing::info!("Press Ctrl+C to stop");
    
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");

    Ok(())
}
