use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Row};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::source::{ConfigSource, ConfigWatcher};

/// SQLite 配置源
pub struct SqliteSource<T> {
    pool: SqlitePool,
    service_name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> SqliteSource<T> {
    pub async fn new(database_url: &str, service_name: String) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;

        // 创建表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_name TEXT NOT NULL,
                config_data TEXT NOT NULL,
                version INTEGER NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                author TEXT,
                comment TEXT
            )
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_configs_service ON configs(service_name)",
        )
        .execute(&pool)
        .await?;

        info!("SQLite config source initialized for service: {}", service_name);

        Ok(Self {
            pool,
            service_name,
            _phantom: std::marker::PhantomData,
        })
    }
}

#[async_trait]
impl<T> ConfigSource<T> for SqliteSource<T>
where
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    async fn load(&self) -> Result<T> {
        debug!("Loading config from SQLite for service: {}", self.service_name);

        let row = sqlx::query(
            "SELECT config_data FROM configs WHERE service_name = ? ORDER BY version DESC LIMIT 1",
        )
        .bind(&self.service_name)
        .fetch_one(&self.pool)
        .await?;

        let config_data: String = row.get("config_data");
        let config: T = serde_json::from_str(&config_data)?;

        Ok(config)
    }

    async fn save(&self, config: &T) -> Result<()> {
        debug!("Saving config to SQLite for service: {}", self.service_name);

        let config_data = serde_json::to_string(config)?;

        // 获取当前最大版本号
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM configs WHERE service_name = ?",
        )
        .bind(&self.service_name)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO configs (service_name, config_data, version, author, comment)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&self.service_name)
        .bind(&config_data)
        .bind(version)
        .bind("system")
        .bind("Config update")
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn watch(&self) -> Result<ConfigWatcher> {
        // SQLite 不支持原生的变更通知
        // 这里返回一个永不触发的 watcher
        // 实际使用中可以通过轮询或外部触发机制实现
        let (_tx, rx) = mpsc::channel(1);
        Ok(ConfigWatcher::new(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        value: String,
    }

    #[tokio::test]
    async fn test_sqlite_source() {
        let source = SqliteSource::new(":memory:", "test_service".to_string())
            .await
            .unwrap();

        let config = TestConfig {
            value: "test".to_string(),
        };

        source.save(&config).await.unwrap();
        let loaded: TestConfig = source.load().await.unwrap();

        assert_eq!(loaded, config);
    }

    #[tokio::test]
    async fn test_sqlite_source_versioning() {
        let source = SqliteSource::new(":memory:", "test_service".to_string())
            .await
            .unwrap();

        // 保存第一个版本
        source
            .save(&TestConfig {
                value: "v1".to_string(),
            })
            .await
            .unwrap();

        // 保存第二个版本
        source
            .save(&TestConfig {
                value: "v2".to_string(),
            })
            .await
            .unwrap();

        // 应该加载最新版本
        let loaded: TestConfig = source.load().await.unwrap();
        assert_eq!(loaded.value, "v2");
    }
}
