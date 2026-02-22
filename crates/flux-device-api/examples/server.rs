use flux_device::DeviceManager;
use flux_device_api::{create_router, AppState};
use sea_orm::Database;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,flux_device_api=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Device Management API Server");

    // 连接数据库（使用 SQLite 用于演示）
    let db = Database::connect("sqlite::memory:").await?;
    
    // 创建表结构
    setup_schema(&db).await?;

    // 创建设备管理器
    let device_manager = Arc::new(DeviceManager::new(Arc::new(db), 30, 60));
    
    // 启动设备监控
    device_manager.start().await;

    // 创建 API 状态
    let state = AppState::new(device_manager);

    // 创建路由
    let app = create_router(state);

    // 启动服务器
    let addr = "0.0.0.0:8080";
    tracing::info!("Listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// 设置数据库表结构
async fn setup_schema(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    use sea_orm::{ConnectionTrait, Statement};
    
    // 创建设备表
    db.execute(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE IF NOT EXISTS devices (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            device_type TEXT NOT NULL,
            protocol TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'Inactive',
            product_id TEXT,
            secret TEXT,
            metadata TEXT,
            tags TEXT,
            group_id TEXT,
            location TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_seen TEXT
        )
        "#.to_string()
    )).await?;
    
    // 创建分组表
    db.execute(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE IF NOT EXISTS device_groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            parent_id TEXT,
            path TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#.to_string()
    )).await?;
    
    // 创建指标表
    db.execute(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE IF NOT EXISTS device_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id TEXT NOT NULL,
            metric_name TEXT NOT NULL,
            metric_value REAL NOT NULL,
            unit TEXT,
            timestamp TEXT NOT NULL
        )
        "#.to_string()
    )).await?;
    
    tracing::info!("Database schema created");
    Ok(())
}
