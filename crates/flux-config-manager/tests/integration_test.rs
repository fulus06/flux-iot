use flux_config_manager::{ConfigManager, FileSource};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AppConfig {
    port: u16,
    host: String,
    max_connections: i32,
}

#[tokio::test]
async fn test_config_manager_full_workflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().with_extension("toml");

    // 创建初始配置
    let initial_config = AppConfig {
        port: 8080,
        host: "localhost".to_string(),
        max_connections: 100,
    };

    // 保存初始配置
    let source = Arc::new(FileSource::new(&path));
    source.save(&initial_config).await.unwrap();

    // 创建配置管理器
    let manager: Arc<ConfigManager<AppConfig>> = Arc::new(ConfigManager::new(source));

    // 加载配置
    let config = manager.load().await.unwrap();
    assert_eq!(config, initial_config);

    // 订阅配置变更
    let mut rx = manager.subscribe().await;

    // 更新配置
    let updated_config = AppConfig {
        port: 9000,
        host: "0.0.0.0".to_string(),
        max_connections: 200,
    };

    manager
        .update(
            updated_config.clone(),
            "test".to_string(),
            "Update port".to_string(),
        )
        .await
        .unwrap();

    // 验证配置已更新
    let current = manager.get().await.unwrap();
    assert_eq!(current, updated_config);

    // 验证收到变更通知
    let change = rx.recv().await.unwrap();
    match change {
        flux_config_manager::ConfigChange::Updated { old, new } => {
            assert_eq!(old, initial_config);
            assert_eq!(new, updated_config);
        }
        _ => panic!("Expected Updated event"),
    }

    // 查看版本历史
    let versions = manager.list_versions().await;
    assert_eq!(versions.len(), 2);

    // 回滚到第一个版本
    manager.rollback(1).await.unwrap();

    let current = manager.get().await.unwrap();
    assert_eq!(current, initial_config);

    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn test_config_hot_reload() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().with_extension("toml");

    // 创建初始配置
    let initial_config = AppConfig {
        port: 8080,
        host: "localhost".to_string(),
        max_connections: 100,
    };

    // 保存初始配置
    let source = Arc::new(FileSource::new(&path));
    source.save(&initial_config).await.unwrap();

    // 创建配置管理器
    let manager: Arc<ConfigManager<AppConfig>> = Arc::new(ConfigManager::new(source.clone()));
    manager.load().await.unwrap();

    // 订阅配置变更
    let mut rx = manager.subscribe().await;

    // 启动热更新监听
    manager.clone().start_watching().await.unwrap();

    // 修改配置文件
    let updated_config = AppConfig {
        port: 9000,
        host: "0.0.0.0".to_string(),
        max_connections: 200,
    };

    source.save(&updated_config).await.unwrap();

    // 等待文件监听器检测到变更
    sleep(Duration::from_millis(500)).await;

    // 验证配置已自动重新加载
    let current = manager.get().await.unwrap();
    assert_eq!(current, updated_config);

    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn test_config_validation() {
    use flux_config_manager::{ConfigValidator, RangeRule};

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().with_extension("toml");

    let source: Arc<FileSource<AppConfig>> = Arc::new(FileSource::new(&path));
    let manager: Arc<ConfigManager<AppConfig>> = Arc::new(ConfigManager::new(source));

    // 创建校验器
    let mut validator = ConfigValidator::new();
    validator.add_rule(Box::new(RangeRule::new(
        "port_range".to_string(),
        "port".to_string(),
        |c: &AppConfig| c.port as i64,
        1024,
        65535,
    )));

    // 有效配置
    let valid_config = AppConfig {
        port: 8080,
        host: "localhost".to_string(),
        max_connections: 100,
    };

    assert!(validator.validate(&valid_config).is_ok());

    // 无效配置
    let invalid_config = AppConfig {
        port: 80,
        host: "localhost".to_string(),
        max_connections: 100,
    };

    assert!(validator.validate(&invalid_config).is_err());

    std::fs::remove_file(&path).ok();
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn test_sqlite_source_integration() {
    use flux_config_manager::SqliteSource;

    let source = Arc::new(
        SqliteSource::new(":memory:", "test_service".to_string())
            .await
            .unwrap(),
    );

    let manager: Arc<ConfigManager<AppConfig>> = Arc::new(ConfigManager::new(source));

    let config = AppConfig {
        port: 8080,
        host: "localhost".to_string(),
        max_connections: 100,
    };

    manager
        .update(config.clone(), "test".to_string(), "Initial".to_string())
        .await
        .unwrap();

    let loaded = manager.get().await.unwrap();
    assert_eq!(loaded, config);

    // 更新配置
    let updated = AppConfig {
        port: 9000,
        host: "0.0.0.0".to_string(),
        max_connections: 200,
    };

    manager
        .update(updated.clone(), "test".to_string(), "Update".to_string())
        .await
        .unwrap();

    let loaded = manager.get().await.unwrap();
    assert_eq!(loaded, updated);

    // 验证版本历史
    let versions = manager.list_versions().await;
    assert_eq!(versions.len(), 2);
}
