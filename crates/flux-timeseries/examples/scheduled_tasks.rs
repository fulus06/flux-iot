use chrono::Duration;
use flux_timeseries::{
    ArchiveDestination, ArchivePolicy, CleanupPolicy, ScheduledTask, TaskScheduler, TimescaleStore,
};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("⏰ FLUX TimeSeries Scheduled Tasks Example\n");

    // 连接数据库
    let database_url = "postgresql://postgres:postgres@localhost:5432/flux_iot";
    let store = TimescaleStore::new(database_url).await?;
    let db = store.connection();
    println!("✅ Connected to TimescaleDB\n");

    // 创建任务调度器
    let mut scheduler = TaskScheduler::new(db.clone().into()).await?;
    println!("📅 Task scheduler created\n");

    // 1. 添加每日归档任务
    println!("📦 Adding daily archive task...");
    let archive_policy = ArchivePolicy {
        table_name: "device_metrics".to_string(),
        archive_older_than: Duration::days(365),
        destination: ArchiveDestination::LocalFile {
            path: "/tmp/flux_archive/".to_string(),
        },
        delete_after_archive: false,
    };

    let daily_archive = ScheduledTask::daily_archive(archive_policy);
    println!("  Task: {}", daily_archive.name);
    println!("  Cron: {} (每天凌晨 2 点)", daily_archive.cron_expression);
    
    let job_id1 = scheduler.add_task(daily_archive).await?;
    println!("  ✓ Job ID: {}\n", job_id1);

    // 2. 添加每日清理任务
    println!("🧹 Adding daily cleanup task...");
    let cleanup_policy = CleanupPolicy::for_metrics();
    let daily_cleanup = ScheduledTask::daily_cleanup(cleanup_policy);
    println!("  Task: {}", daily_cleanup.name);
    println!("  Cron: {} (每天凌晨 1 点)", daily_cleanup.cron_expression);
    
    let job_id2 = scheduler.add_task(daily_cleanup).await?;
    println!("  ✓ Job ID: {}\n", job_id2);

    // 3. 添加每周归档任务
    println!("📅 Adding weekly archive task...");
    let weekly_policy = ArchivePolicy {
        table_name: "device_logs".to_string(),
        archive_older_than: Duration::days(90),
        destination: ArchiveDestination::LocalFile {
            path: "/tmp/flux_archive/".to_string(),
        },
        delete_after_archive: false,
    };

    let weekly_archive = ScheduledTask::weekly_archive(weekly_policy);
    println!("  Task: {}", weekly_archive.name);
    println!("  Cron: {} (每周日凌晨 3 点)", weekly_archive.cron_expression);
    
    let job_id3 = scheduler.add_task(weekly_archive).await?;
    println!("  ✓ Job ID: {}\n", job_id3);

    // 4. 添加每小时降采样刷新任务
    println!("📊 Adding hourly downsample refresh task...");
    let refresh_task = ScheduledTask::hourly_downsample_refresh("device_metrics_1h".to_string());
    println!("  Task: {}", refresh_task.name);
    println!("  Cron: {} (每小时整点)", refresh_task.cron_expression);
    
    let job_id4 = scheduler.add_task(refresh_task).await?;
    println!("  ✓ Job ID: {}\n", job_id4);

    // 5. 自定义 Cron 任务
    println!("🎨 Adding custom cron task...");
    let custom_policy = CleanupPolicy::for_logs();
    let custom_task = ScheduledTask::new(
        "Custom Cleanup".to_string(),
        "0 30 * * * *".to_string(), // 每小时 30 分
        flux_timeseries::TaskType::Cleanup(custom_policy),
    );
    println!("  Task: {}", custom_task.name);
    println!("  Cron: {} (每小时 30 分)", custom_task.cron_expression);
    
    let job_id5 = scheduler.add_task(custom_task).await?;
    println!("  ✓ Job ID: {}\n", job_id5);

    // 启动调度器
    println!("🚀 Starting scheduler...");
    scheduler.start().await?;
    println!("  ✓ Scheduler is running\n");

    // 任务已添加
    println!("📋 All tasks have been scheduled\n");

    // Cron 表达式说明
    println!("📖 Cron Expression Format:");
    println!("  ┌─────────── second (0-59)");
    println!("  │ ┌───────── minute (0-59)");
    println!("  │ │ ┌─────── hour (0-23)");
    println!("  │ │ │ ┌───── day of month (1-31)");
    println!("  │ │ │ │ ┌─── month (1-12)");
    println!("  │ │ │ │ │ ┌─ day of week (0-6, 0=Sunday)");
    println!("  │ │ │ │ │ │");
    println!("  * * * * * *");
    println!();

    println!("📚 Common Cron Examples:");
    println!("  0 0 2 * * *     - 每天凌晨 2 点");
    println!("  0 0 * * * *     - 每小时整点");
    println!("  0 30 * * * *    - 每小时 30 分");
    println!("  0 0 3 * * 0     - 每周日凌晨 3 点");
    println!("  0 0 4 1 * *     - 每月 1 号凌晨 4 点");
    println!("  0 */15 * * * *  - 每 15 分钟");
    println!();

    println!("⏳ Scheduler will run for 60 seconds...");
    println!("   (In production, the scheduler runs indefinitely)\n");

    // 运行 60 秒后停止（演示用）
    sleep(std::time::Duration::from_secs(60)).await;

    // 停止调度器
    println!("\n🛑 Stopping scheduler...");
    scheduler.shutdown().await?;
    println!("  ✓ Scheduler stopped\n");

    println!("✨ Scheduled tasks example completed!");

    Ok(())
}
