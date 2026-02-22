// 通用测试工具模块
// 提供跨测试复用的辅助函数和 Mock 对象

use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;
use flux_storage::StorageManager;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Schema};
use std::sync::Arc;
use tokio::sync::watch;

/// 测试用的应用配置
pub fn test_app_config() -> flux_server::config::AppConfig {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/flux_test".to_string());
    
    flux_server::config::AppConfig {
        server: flux_server::config::ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        database: flux_server::config::DatabaseConfig {
            url: database_url,
        },
        eventbus: flux_server::config::EventBusConfig {
            capacity: 100,
        },
        plugins: flux_server::config::PluginsConfig {
            directory: "./test_plugins".to_string(),
        },
        gb28181: flux_server::config::Gb28181Config {
            enabled: false,
            ..Default::default()
        },
    }
}

/// 创建测试数据库连接（PostgreSQL）
pub async fn create_test_db() -> DatabaseConnection {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/flux_test".to_string());
    Database::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL test database")
}

/// 初始化测试数据库表结构
pub async fn init_test_schema(db: &DatabaseConnection) -> anyhow::Result<()> {
    use flux_core::entity::{devices, events, rules};
    
    let schema = Schema::new(db.get_database_backend());
    let builder = db.get_database_backend();

    // 创建 rules 表
    let stmt = schema.create_table_from_entity(rules::Entity).if_not_exists();
    db.execute(builder.build(&stmt)).await?;

    // 创建 events 表
    let stmt = schema.create_table_from_entity(events::Entity).if_not_exists();
    db.execute(builder.build(&stmt)).await?;

    // 创建 devices 表
    let stmt = schema.create_table_from_entity(devices::Entity).if_not_exists();
    db.execute(builder.build(&stmt)).await?;

    Ok(())
}

/// 创建测试用的 AppState
pub async fn create_test_state() -> Arc<flux_server::AppState> {
    let event_bus = Arc::new(EventBus::new(100));
    let plugin_manager = Arc::new(PluginManager::new().expect("Failed to create PluginManager"));
    let script_engine = Arc::new(ScriptEngine::new());
    let storage_manager = Arc::new(StorageManager::new());
    let db = create_test_db().await;
    
    init_test_schema(&db).await.expect("Failed to init schema");
    
    let (_tx, rx) = watch::channel(test_app_config());

    Arc::new(flux_server::AppState {
        event_bus,
        plugin_manager,
        script_engine,
        storage_manager,
        db,
        config_db: None,
        config: rx,
        gb28181_sip: None,
        gb28181_backend: None,
    })
}

/// 等待异步事件（带超时）
pub async fn wait_for_event<T, F>(mut rx: tokio::sync::broadcast::Receiver<T>, predicate: F, timeout_ms: u64) -> Option<T>
where
    F: Fn(&T) -> bool,
    T: Clone,
{
    let timeout = tokio::time::Duration::from_millis(timeout_ms);
    let deadline = tokio::time::Instant::now() + timeout;
    
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(msg)) => {
                if predicate(&msg) {
                    return Some(msg);
                }
            }
            Ok(Err(_)) => return None,
            Err(_) => return None,
        }
    }
}

/// 生成随机测试 ID
pub fn random_test_id(prefix: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{}_{}", prefix, timestamp)
}

/// Mock MQTT 客户端
#[cfg(feature = "mqtt-test")]
pub mod mqtt {
    use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, QoS};
    use tokio::time::Duration;

    pub async fn create_test_client(client_id: &str) -> (AsyncClient, EventLoop) {
        let mut mqttoptions = MqttOptions::new(client_id, "127.0.0.1", 1883);
        mqttoptions.set_keep_alive(Duration::from_secs(5));
        AsyncClient::new(mqttoptions, 10)
    }

    pub async fn publish_and_wait(
        client: &AsyncClient,
        topic: &str,
        payload: &[u8],
        qos: QoS,
    ) -> anyhow::Result<()> {
        client.publish(topic, qos, false, payload).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}

/// Mock HTTP 客户端
pub mod http {
    use reqwest::Client;
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    pub fn test_client() -> Client {
        Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client")
    }

    pub async fn post_json<T: Serialize, R: DeserializeOwned>(
        base_url: &str,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        let client = test_client();
        let url = format!("{}{}", base_url, path);
        let response = client.post(&url).json(body).send().await?;
        Ok(response.json().await?)
    }

    pub async fn get_json<R: DeserializeOwned>(
        base_url: &str,
        path: &str,
    ) -> anyhow::Result<R> {
        let client = test_client();
        let url = format!("{}{}", base_url, path);
        let response = client.get(&url).send().await?;
        Ok(response.json().await?)
    }
}

/// 临时目录管理
pub mod temp {
    use std::path::PathBuf;
    use tempfile::TempDir;

    pub struct TestDir {
        _dir: TempDir,
        pub path: PathBuf,
    }

    impl TestDir {
        pub fn new() -> Self {
            let dir = TempDir::new().expect("Failed to create temp dir");
            let path = dir.path().to_path_buf();
            Self { _dir: dir, path }
        }

        pub fn path(&self) -> &PathBuf {
            &self.path
        }
    }
}
