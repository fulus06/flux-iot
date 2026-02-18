use crate::config::AppConfig;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use config::{Config, File, FileFormat};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value};
use std::time::UNIX_EPOCH;

#[async_trait]
pub trait AppConfigProvider: Send + Sync {
    async fn load(&self) -> Result<AppConfig>;

    /// 用于检测配置是否变更的版本号（单调递增/变化即可）
    async fn version(&self) -> Result<i64>;
}

pub struct FileConfigProvider {
    path: String,
}

impl FileConfigProvider {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl AppConfigProvider for FileConfigProvider {
    async fn load(&self) -> Result<AppConfig> {
        let settings = Config::builder()
            .add_source(File::with_name(&self.path))
            .build()?;

        let app_config: AppConfig = settings.try_deserialize()?;
        Ok(app_config)
    }

    async fn version(&self) -> Result<i64> {
        let meta = std::fs::metadata(&self.path)
            .map_err(|e| anyhow!("Failed to read config metadata {}: {}", self.path, e))?;
        let modified = meta
            .modified()
            .map_err(|e| anyhow!("Failed to read config mtime {}: {}", self.path, e))?;
        let ms = modified
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("Invalid mtime for {}: {}", self.path, e))?
            .as_millis();
        Ok(ms as i64)
    }
}

pub struct DbConfigProvider {
    db: DatabaseConnection,
    seed_file: Option<String>,
}

impl DbConfigProvider {
    pub fn new(db: DatabaseConnection, seed_file: Option<String>) -> Self {
        Self { db, seed_file }
    }

    async fn ensure_table(&self) -> Result<()> {
        let backend = self.db.get_database_backend();

        let sql = match backend {
            DbBackend::Sqlite => {
                "CREATE TABLE IF NOT EXISTS app_config (\
                    id INTEGER PRIMARY KEY AUTOINCREMENT,\
                    content TEXT NOT NULL,\
                    updated_at INTEGER NOT NULL\
                )"
            }
            DbBackend::Postgres => {
                "CREATE TABLE IF NOT EXISTS app_config (\
                    id BIGSERIAL PRIMARY KEY,\
                    content TEXT NOT NULL,\
                    updated_at BIGINT NOT NULL\
                )"
            }
            DbBackend::MySql => {
                "CREATE TABLE IF NOT EXISTS app_config (\
                    id BIGINT AUTO_INCREMENT PRIMARY KEY,\
                    content TEXT NOT NULL,\
                    updated_at BIGINT NOT NULL\
                )"
            }
        };

        self.db
            .execute(Statement::from_string(backend, sql.to_string()))
            .await?;

        Ok(())
    }

    async fn try_load_from_db(&self) -> Result<Option<String>> {
        let backend = self.db.get_database_backend();
        let stmt = Statement::from_string(
            backend,
            "SELECT content FROM app_config ORDER BY updated_at DESC LIMIT 1".to_string(),
        );

        let row_opt = self.db.query_one(stmt).await?;
        let Some(row) = row_opt else {
            return Ok(None);
        };

        let content: String = row.try_get("", "content")?;
        Ok(Some(content))
    }

    async fn try_load_version(&self) -> Result<i64> {
        let backend = self.db.get_database_backend();
        let stmt = Statement::from_string(
            backend,
            "SELECT updated_at FROM app_config ORDER BY updated_at DESC LIMIT 1".to_string(),
        );

        let row_opt = self.db.query_one(stmt).await?;
        let Some(row) = row_opt else {
            return Ok(0);
        };

        let updated_at: i64 = row.try_get("", "updated_at")?;
        Ok(updated_at)
    }

    async fn seed_from_file_if_needed(&self) -> Result<()> {
        let Some(seed_path) = &self.seed_file else {
            return Ok(());
        };

        if self.try_load_from_db().await?.is_some() {
            return Ok(());
        }

        let content = std::fs::read_to_string(seed_path)
            .map_err(|e| anyhow!("Failed to read seed config file {}: {}", seed_path, e))?;

        let backend = self.db.get_database_backend();
        let now = chrono::Utc::now().timestamp_millis();

        let insert_sql = match backend {
            DbBackend::Postgres => "INSERT INTO app_config (content, updated_at) VALUES ($1, $2)",
            DbBackend::Sqlite | DbBackend::MySql => {
                "INSERT INTO app_config (content, updated_at) VALUES (?, ?)"
            }
        };

        let stmt = Statement::from_sql_and_values(
            backend,
            insert_sql,
            vec![
                Value::String(Some(Box::new(content))),
                Value::BigInt(Some(now)),
            ],
        );

        self.db.execute(stmt).await?;
        Ok(())
    }
}

#[async_trait]
impl AppConfigProvider for DbConfigProvider {
    async fn load(&self) -> Result<AppConfig> {
        self.ensure_table().await?;
        self.seed_from_file_if_needed().await?;

        let content = self
            .try_load_from_db()
            .await?
            .ok_or_else(|| anyhow!("No app_config found in database"))?;

        let settings = Config::builder()
            .add_source(File::from_str(&content, FileFormat::Toml))
            .build()?;

        let app_config: AppConfig = settings.try_deserialize()?;
        Ok(app_config)
    }

    async fn version(&self) -> Result<i64> {
        self.ensure_table().await?;
        self.try_load_version().await
    }
}
