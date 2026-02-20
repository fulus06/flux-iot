use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPool, Row};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::source::{ConfigSource, ConfigWatcher};

/// PostgreSQL 配置源
pub struct PostgresSource<T> {
    pool: PgPool,
    service_name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> PostgresSource<T> {
    pub async fn new(database_url: &str, service_name: String) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;

        // 创建表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS configs (
                id SERIAL PRIMARY KEY,
                service_name VARCHAR(255) NOT NULL,
                config_data JSONB NOT NULL,
                version BIGINT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                author VARCHAR(255),
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

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_configs_data ON configs USING GIN(config_data)",
        )
        .execute(&pool)
        .await?;

        info!("PostgreSQL config source initialized for service: {}", service_name);

        Ok(Self {
            pool,
            service_name,
            _phantom: std::marker::PhantomData,
        })
    }
}

#[async_trait]
impl<T> ConfigSource<T> for PostgresSource<T>
where
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    async fn load(&self) -> Result<T> {
        debug!("Loading config from PostgreSQL for service: {}", self.service_name);

        let row = sqlx::query(
            "SELECT config_data FROM configs WHERE service_name = $1 ORDER BY version DESC LIMIT 1",
        )
        .bind(&self.service_name)
        .fetch_one(&self.pool)
        .await?;

        let config_data: serde_json::Value = row.get("config_data");
        let config: T = serde_json::from_value(config_data)?;

        Ok(config)
    }

    async fn save(&self, config: &T) -> Result<()> {
        debug!("Saving config to PostgreSQL for service: {}", self.service_name);

        let config_data = serde_json::to_value(config)?;

        // 获取当前最大版本号
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM configs WHERE service_name = $1",
        )
        .bind(&self.service_name)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO configs (service_name, config_data, version, author, comment)
            VALUES ($1, $2, $3, $4, $5)
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
        // PostgreSQL 支持 LISTEN/NOTIFY 机制
        // 这里简化实现，返回一个永不触发的 watcher
        // 实际使用中可以通过 LISTEN/NOTIFY 实现实时通知
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

    // 注意：这些测试需要实际的 PostgreSQL 数据库
    // 在 CI 环境中应该使用 testcontainers 或类似工具

    #[tokio::test]
    #[ignore] // 需要 PostgreSQL 数据库
    async fn test_postgres_source() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/test".to_string());

        let source = PostgresSource::new(&database_url, "test_service".to_string())
            .await
            .unwrap();

        let config = TestConfig {
            value: "test".to_string(),
        };

        source.save(&config).await.unwrap();
        let loaded: TestConfig = source.load().await.unwrap();

        assert_eq!(loaded, config);
    }
}
