use flux_core::bus::EventBus;
use flux_core::traits::auth::Authenticator;
use flux_mqtt::{manager::MqttManager, start_broker};
use std::sync::Arc;
use std::time::Duration;
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

    tracing::info!("Starting MQTT Broker with Metrics Example");

    // 创建 EventBus
    let event_bus = Arc::new(EventBus::new(1000));

    // 创建认证器
    let authenticator = Arc::new(SimpleAuthenticator);

    // 创建管理器（用于访问指标）
    let manager = Arc::new(MqttManager::new());
    let metrics_manager = manager.clone();

    // 启动 MQTT broker
    start_broker(event_bus.clone(), authenticator);

    // 启动指标报告任务
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;

            let snapshot = metrics_manager.metrics().snapshot();
            
            tracing::info!("=== MQTT Metrics ===");
            tracing::info!("Connections:");
            tracing::info!("  Current: {}", snapshot.connections_current);
            tracing::info!("  Total: {}", snapshot.connections_total);
            tracing::info!("  Peak: {}", snapshot.connections_peak);
            
            tracing::info!("Messages:");
            tracing::info!("  Published: {}", snapshot.messages_published);
            tracing::info!("  Received: {}", snapshot.messages_received);
            tracing::info!("  Dropped: {}", snapshot.messages_dropped);
            
            tracing::info!("Bytes:");
            tracing::info!("  Sent: {}", snapshot.bytes_sent);
            tracing::info!("  Received: {}", snapshot.bytes_received);
            
            tracing::info!("QoS:");
            tracing::info!("  QoS 0: {}", snapshot.qos0_messages);
            tracing::info!("  QoS 1: {}", snapshot.qos1_messages);
            tracing::info!("  QoS 2: {}", snapshot.qos2_messages);
            
            tracing::info!("Other:");
            tracing::info!("  Retained: {}", snapshot.retained_messages);
            tracing::info!("  Subscriptions: {}", snapshot.subscriptions_current);
            tracing::info!("  Uptime: {:?}", snapshot.uptime);
            tracing::info!("===================");
        }
    });

    // 可选：启动 HTTP 服务器暴露 Prometheus 指标
    let http_manager = manager.clone();
    tokio::spawn(async move {
        use std::net::SocketAddr;
        use tokio::net::TcpListener;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let addr: SocketAddr = "0.0.0.0:9090".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();
        
        tracing::info!("Prometheus metrics available at http://0.0.0.0:9090/metrics");

        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                let manager = http_manager.clone();
                tokio::spawn(async move {
                    let mut buffer = [0; 1024];
                    if socket.read(&mut buffer).await.is_ok() {
                        let prometheus = manager.metrics().export_prometheus();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                            prometheus.len(),
                            prometheus
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                });
            }
        }
    });

    // 订阅 EventBus 消息
    let mut rx = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            tracing::debug!(
                topic = %msg.topic,
                "Message from EventBus"
            );
        }
    });

    // 保持运行
    tracing::info!("MQTT Broker is running on port 1883");
    tracing::info!("Metrics endpoint: http://localhost:9090/metrics");
    tracing::info!("Press Ctrl+C to stop");
    
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");

    Ok(())
}
