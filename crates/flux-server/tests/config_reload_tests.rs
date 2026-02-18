use flux_server::config::AppConfig;
use flux_server::config_manager::ConfigManager;
use flux_server::config_provider::{AppConfigProvider, DbConfigProvider, FileConfigProvider};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, Value};
use std::sync::Arc;
use std::time::Duration;

fn minimal_config_toml(port: u16, eventbus_capacity: usize) -> String {
    format!(
        r#"
[server]
host = "127.0.0.1"
port = {port}

[database]
url = "sqlite::memory:"

[plugins]
directory = "plugins"

[eventbus]
capacity = {eventbus_capacity}
"#
    )
}

async fn wait_for_capacity(
    rx: &mut tokio::sync::watch::Receiver<AppConfig>,
    expected: usize,
    timeout: Duration,
) -> anyhow::Result<()> {
    let fut = async {
        loop {
            let cap = rx.borrow().eventbus.capacity;
            if cap == expected {
                return Ok(());
            }
            rx.changed()
                .await
                .map_err(|e| anyhow::anyhow!("watch channel closed: {}", e))?;
        }
    };

    tokio::time::timeout(timeout, fut)
        .await
        .map_err(|_| anyhow::anyhow!("timeout waiting for config update"))?
}

#[tokio::test]
async fn test_file_config_hot_reload() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("config.toml");

    std::fs::write(&path, minimal_config_toml(3001, 111))?;

    let provider: Arc<dyn AppConfigProvider> = Arc::new(FileConfigProvider::new(
        path.to_string_lossy().to_string(),
    ));

    let initial = provider.load().await?;
    let ver = provider.version().await?;

    let mgr = Arc::new(ConfigManager::new(provider, initial, ver));
    mgr.clone().start_polling(Duration::from_millis(50));

    let mut rx = mgr.subscribe();

    // 修改文件，触发 reload
    tokio::time::sleep(Duration::from_millis(60)).await;
    std::fs::write(&path, minimal_config_toml(3002, 222))?;

    wait_for_capacity(&mut rx, 222, Duration::from_secs(2)).await?;
    Ok(())
}

#[tokio::test]
async fn test_sqlite_config_hot_reload() -> anyhow::Result<()> {
    let db = Database::connect("sqlite::memory:").await?;

    // Create app_config table.
    let backend = db.get_database_backend();
    let create_sql = match backend {
        DbBackend::Sqlite => {
            "CREATE TABLE IF NOT EXISTS app_config (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                content TEXT NOT NULL,\
                updated_at INTEGER NOT NULL\
            )"
        }
        _ => return Err(anyhow::anyhow!("unexpected backend for sqlite test")),
    };

    db.execute(Statement::from_string(backend, create_sql.to_string()))
        .await?;

    // Seed v1
    let now1 = 1000_i64;
    let toml1 = minimal_config_toml(3001, 333);
    db.execute(Statement::from_sql_and_values(
        backend,
        "INSERT INTO app_config (content, updated_at) VALUES (?, ?)",
        vec![
            Value::String(Some(Box::new(toml1))),
            Value::BigInt(Some(now1)),
        ],
    ))
    .await?;

    let provider: Arc<dyn AppConfigProvider> = Arc::new(DbConfigProvider::new(db.clone(), None));
    let initial = provider.load().await?;
    let ver = provider.version().await?;

    let mgr = Arc::new(ConfigManager::new(provider, initial, ver));
    mgr.clone().start_polling(Duration::from_millis(50));

    let mut rx = mgr.subscribe();

    // Seed v2 (newer updated_at)
    let now2 = 2000_i64;
    let toml2 = minimal_config_toml(3002, 444);
    tokio::time::sleep(Duration::from_millis(60)).await;
    db.execute(Statement::from_sql_and_values(
        backend,
        "INSERT INTO app_config (content, updated_at) VALUES (?, ?)",
        vec![
            Value::String(Some(Box::new(toml2))),
            Value::BigInt(Some(now2)),
        ],
    ))
    .await?;

    wait_for_capacity(&mut rx, 444, Duration::from_secs(2)).await?;
    Ok(())
}

#[tokio::test]
async fn test_postgres_config_hot_reload_optional() -> anyhow::Result<()> {
    let pg_url = match std::env::var("FLUX_TEST_PG_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => return Ok(()),
    };

    let db = Database::connect(&pg_url).await?;

    // Ensure app_config exists.
    let backend = db.get_database_backend();
    let create_sql = "CREATE TABLE IF NOT EXISTS app_config (\
        id BIGSERIAL PRIMARY KEY,\
        content TEXT NOT NULL,\
        updated_at BIGINT NOT NULL\
    )";
    db.execute(Statement::from_string(backend, create_sql.to_string()))
        .await?;

    // Seed v1
    let now1 = chrono::Utc::now().timestamp_millis();
    let toml1 = minimal_config_toml(3001, 555);
    db.execute(Statement::from_sql_and_values(
        backend,
        "INSERT INTO app_config (content, updated_at) VALUES ($1, $2)",
        vec![
            Value::String(Some(Box::new(toml1))),
            Value::BigInt(Some(now1)),
        ],
    ))
    .await?;

    let provider: Arc<dyn AppConfigProvider> = Arc::new(DbConfigProvider::new(db.clone(), None));
    let initial = provider.load().await?;
    let ver = provider.version().await?;

    let mgr = Arc::new(ConfigManager::new(provider, initial, ver));
    mgr.clone().start_polling(Duration::from_millis(100));

    let mut rx = mgr.subscribe();

    let now2 = now1 + 1000;
    let toml2 = minimal_config_toml(3002, 666);
    db.execute(Statement::from_sql_and_values(
        backend,
        "INSERT INTO app_config (content, updated_at) VALUES ($1, $2)",
        vec![
            Value::String(Some(Box::new(toml2))),
            Value::BigInt(Some(now2)),
        ],
    ))
    .await?;

    wait_for_capacity(&mut rx, 666, Duration::from_secs(3)).await?;
    Ok(())
}
