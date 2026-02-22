use chrono::Duration;
use flux_timeseries::{
    ArchiveDestination, ArchivePolicy, CleanupManager, CleanupPolicy, DataArchiver,
    DownsampleManager, DownsamplePolicy, TimescaleStore,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🗄️  FLUX TimeSeries Archive & Cleanup Example\n");

    // 连接数据库
    let database_url = "postgresql://postgres:postgres@localhost:5432/flux_iot";
    let store = TimescaleStore::new(database_url).await?;
    let db = store.connection();
    println!("✅ Connected to TimescaleDB\n");

    // 1. 数据降采样
    println!("📊 Creating downsample views...");
    let downsample_mgr = DownsampleManager::new(db.clone().into());

    // 创建日聚合视图
    let daily_policy = DownsamplePolicy::daily();
    downsample_mgr.create_downsample_view(&daily_policy).await?;
    downsample_mgr.add_refresh_policy(&daily_policy).await?;
    downsample_mgr.add_retention_policy(&daily_policy).await?;
    println!("  ✓ Daily downsample view created");

    // 创建周聚合视图
    let weekly_policy = DownsamplePolicy::weekly();
    downsample_mgr.create_downsample_view(&weekly_policy).await?;
    downsample_mgr.add_refresh_policy(&weekly_policy).await?;
    println!("  ✓ Weekly downsample view created");

    // 列出所有降采样视图
    let views = downsample_mgr.list_views().await?;
    println!("  ✓ Total downsample views: {}", views.len());
    for view in &views {
        println!("    - {}", view);
    }
    println!();

    // 2. 存储统计
    println!("📈 Getting storage statistics...");
    let cleanup_mgr = CleanupManager::new(db.clone().into());
    let stats = cleanup_mgr.get_storage_stats().await?;
    
    println!("  ✓ Total size: {:.2} MB", stats.total_size_mb);
    println!("  ✓ Compressed size: {:.2} MB", stats.compressed_size_mb);
    println!("  ✓ Compression ratio: {:.2}x", stats.compression_ratio);
    println!("  ✓ Chunk count: {}", stats.chunk_count);
    println!("  Table sizes:");
    for (table, size) in &stats.table_sizes {
        println!("    - {}: {:.2} MB", table, size);
    }
    println!();

    // 3. 数据清理
    println!("🧹 Running cleanup tasks...");
    
    // 清理指标数据（保留90天）
    let metrics_policy = CleanupPolicy::for_metrics();
    let cleanup_stats = cleanup_mgr.cleanup(&metrics_policy).await?;
    println!(
        "  ✓ Metrics cleanup: {} rows deleted, {:.2} MB freed",
        cleanup_stats.deleted_rows, cleanup_stats.freed_space_mb
    );

    // 清理日志数据（保留30天）
    let logs_policy = CleanupPolicy::for_logs();
    let cleanup_stats = cleanup_mgr.cleanup(&logs_policy).await?;
    println!(
        "  ✓ Logs cleanup: {} rows deleted, {:.2} MB freed",
        cleanup_stats.deleted_rows, cleanup_stats.freed_space_mb
    );
    println!();

    // 4. 数据归档
    println!("📦 Running archive task...");
    let archiver = DataArchiver::new(db.clone().into());
    
    let archive_policy = ArchivePolicy {
        table_name: "device_metrics".to_string(),
        archive_older_than: Duration::days(365), // 归档1年前的数据
        destination: ArchiveDestination::LocalFile {
            path: "/tmp/flux_iot_archive.json".to_string(),
        },
        delete_after_archive: false, // 不删除原始数据
    };

    let archive_stats = archiver.archive(&archive_policy).await?;
    println!(
        "  ✓ Archived {} rows ({:.2} MB) to {}",
        archive_stats.archived_rows,
        archive_stats.archive_size_mb,
        archive_stats.destination
    );
    println!();

    println!("✨ Archive & Cleanup example completed successfully!");

    Ok(())
}
