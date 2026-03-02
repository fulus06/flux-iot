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
        // 使用 PostgreSQL LISTEN/NOTIFY 实现配置热重载
        let (tx, rx) = mpsc::channel(100);
        let pool = self.pool.clone();
        let table_name = self.table_name.clone();
        
        // 启动监听任务
        tokio::spawn(async move {
            if let Err(e) = Self::listen_for_changes(pool, table_name, tx).await {
                tracing::error!("Config watch task failed: {}", e);
            }
        });
        
        Ok(ConfigWatcher::new(rx))
    }
    
    /// 监听配置变更
    async fn listen_for_changes(
        pool: PgPool,
        table_name: String,
        tx: mpsc::Sender<ConfigEvent>,
    ) -> Result<()> {
        use sqlx::postgres::PgListener;
        
        // 创建监听器
        let mut listener = PgListener::connect_with(&pool).await?;
        
        // 监听通道
        let channel_name = format!("{}_changes", table_name);
        listener.listen(&channel_name).await?;
        
        tracing::info!(
            channel = %channel_name,
            "Started listening for configuration changes"
        );
        
        // 持续监听通知
        loop {
            match listener.recv().await {
                Ok(notification) => {
                    tracing::debug!(
                        payload = %notification.payload(),
                        "Received configuration change notification"
                    );
                    
                    // 解析通知负载
                    if let Ok(event) = serde_json::from_str::<ConfigEvent>(notification.payload()) {
                        if let Err(e) = tx.send(event).await {
                            tracing::error!("Failed to send config event: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error receiving notification: {}", e);
                    // 短暂延迟后重试
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
        
        Ok(())
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
