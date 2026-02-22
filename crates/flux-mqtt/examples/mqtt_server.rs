use flux_core::bus::EventBus;
use flux_core::traits::auth::Authenticator;
use flux_mqtt::{start_broker, start_broker_with_tls, tls::TlsConfig};
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
        // 简单认证：允许所有客户端
        Ok(true)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info,flux_mqtt=debug")
        .init();

    tracing::info!("Starting MQTT Broker Example");

    // 创建 EventBus (容量 1000)
    let event_bus = Arc::new(EventBus::new(1000));

    // 创建认证器
    let authenticator = Arc::new(SimpleAuthenticator);

    // 检查是否启用 TLS
    let use_tls = std::env::var("MQTT_TLS_ENABLED").unwrap_or_default() == "true";

    if use_tls {
        // 使用 TLS
        let cert_path =
            std::env::var("MQTT_CERT_PATH").unwrap_or_else(|_| "certs/server.crt".to_string());
        let key_path =
            std::env::var("MQTT_KEY_PATH").unwrap_or_else(|_| "certs/server.key".to_string());

        tracing::info!(
            cert_path = %cert_path,
            key_path = %key_path,
            "Starting MQTT broker with TLS"
        );

        let tls_config = TlsConfig::new(cert_path, key_path);
        start_broker_with_tls(event_bus.clone(), authenticator, Some(tls_config));
    } else {
        // 不使用 TLS
        tracing::info!("Starting MQTT broker without TLS");
        start_broker(event_bus.clone(), authenticator);
    }

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
    tracing::info!("MQTT Broker is running. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");

    Ok(())
}
