use chrono::Duration;
use flux_timeseries::{ArchiveDestination, ArchivePolicy, DataArchiver};
use sea_orm::Database;
use std::sync::Arc;

/// 示例：从配置文件创建归档器
/// 
/// 这个示例展示了如何使用配置文件中的设置来创建和使用 DataArchiver
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 连接数据库
    let db = Database::connect("postgresql://postgres:postgres@localhost:5432/flux_iot").await?;
    let db = Arc::new(db);

    // 方式 1: 使用默认配置（本地存储）
    println!("=== 方式 1: 默认配置 ===");
    let archiver = DataArchiver::new(db.clone());
    println!("使用默认路径: /var/lib/flux-iot/archive");

    // 方式 2: 从配置文件读取路径
    println!("\n=== 方式 2: 配置文件路径 ===");
    let archive_path = "/data/archive".to_string(); // 从配置文件读取
    let archiver = DataArchiver::new_with_path(db.clone(), archive_path.into());
    println!("使用配置路径: /data/archive");

    // 方式 3: 使用 from_config（推荐）
    #[cfg(feature = "s3")]
    {
        println!("\n=== 方式 3: from_config (S3) ===");
        use flux_storage::S3Config;
        
        // 从配置文件读取 S3 配置
        let s3_config = Some(S3Config {
            bucket: "flux-archive".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            prefix: Some("timeseries".to_string()),
        });
        
        let archiver = DataArchiver::from_config(
            db.clone(),
            "/var/lib/flux-iot/archive".to_string(),
            s3_config,
        ).await?;
        println!("使用 S3 存储");
    }

    #[cfg(not(feature = "s3"))]
    {
        println!("\n=== 方式 3: from_config (本地) ===");
        let archiver = DataArchiver::from_config(
            db.clone(),
            "/var/lib/flux-iot/archive".to_string(),
            None,
        ).await?;
        println!("使用本地存储（S3 feature 未启用）");
    }

    // 创建归档策略
    let policy = ArchivePolicy {
        table_name: "device_metrics".to_string(),
        archive_older_than: Duration::days(30),
        destination: ArchiveDestination::LocalFile {
            path: "/var/lib/flux-iot/archive".to_string(),
        },
        delete_after_archive: false,
    };

    println!("\n=== 归档策略 ===");
    println!("表名: {}", policy.table_name);
    println!("归档阈值: {} 天", 30);
    println!("删除原始数据: {}", policy.delete_after_archive);

    // 执行归档（注释掉以避免实际执行）
    // let stats = archiver.archive(&policy).await?;
    // println!("\n归档完成:");
    // println!("  归档行数: {}", stats.archived_rows);
    // println!("  文件大小: {:.2} MB", stats.archive_size_mb);
    // println!("  执行时间: {} ms", stats.execution_time_ms);

    Ok(())
}
