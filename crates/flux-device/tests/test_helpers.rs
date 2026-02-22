use sea_orm::{Database, DatabaseConnection, DbErr};

/// 创建测试用的 SQLite 数据库连接
pub async fn create_test_db() -> Result<DatabaseConnection, DbErr> {
    // 使用内存 SQLite 数据库
    let db = Database::connect("sqlite::memory:").await?;
    
    // 创建表结构
    setup_schema(&db).await?;
    
    Ok(db)
}

/// 设置数据库表结构
async fn setup_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    use sea_orm::Statement;
    use sea_orm::ConnectionTrait;
    
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
    
    // 创建设备分组表
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
    
    // 创建设备状态历史表
    db.execute(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE IF NOT EXISTS device_status_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id TEXT NOT NULL,
            status TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            metadata TEXT
        )
        "#.to_string()
    )).await?;
    
    // 创建设备指标表
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
    
    Ok(())
}
