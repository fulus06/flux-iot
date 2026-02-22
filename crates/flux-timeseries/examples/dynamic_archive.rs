use chrono::{Duration, Utc};
use flux_timeseries::{ArchiveDestination, ArchivePolicy, DataArchiver, TimescaleStore};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("📦 FLUX TimeSeries Dynamic Archive Example\n");

    // 连接数据库
    let database_url = "postgresql://postgres:postgres@localhost:5432/flux_iot";
    let store = TimescaleStore::new(database_url).await?;
    let db = store.connection();
    println!("✅ Connected to TimescaleDB\n");

    let archiver = DataArchiver::new(db.clone().into());

    // 示例 1: 使用目录路径，自动生成文件名
    println!("📂 Example 1: Auto-generate filename from directory");
    let policy1 = ArchivePolicy {
        table_name: "device_metrics".to_string(),
        archive_older_than: Duration::days(365),
        destination: ArchiveDestination::LocalFile {
            path: "/tmp/flux_archive/".to_string(), // 目录路径，以 / 结尾
        },
        delete_after_archive: false,
    };

    // 生成的文件名示例: /tmp/flux_archive/device_metrics_20260222_180000.json
    let generated_filename = policy1.generate_filename("/tmp/flux_archive");
    println!("  Generated filename: {}", generated_filename);

    let stats1 = archiver.archive(&policy1).await?;
    println!(
        "  ✓ Archived {} rows to auto-generated file\n",
        stats1.archived_rows
    );

    // 示例 2: 按日期生成文件名
    println!("📅 Example 2: Generate daily filename");
    let now = Utc::now();
    let daily_filename = policy1.generate_daily_filename("/tmp/flux_archive", now);
    println!("  Daily filename: {}", daily_filename);
    // 示例: /tmp/flux_archive/device_metrics_2026-02-22.json

    let policy2 = ArchivePolicy {
        table_name: "device_metrics".to_string(),
        archive_older_than: Duration::days(365),
        destination: ArchiveDestination::LocalFile {
            path: daily_filename.clone(),
        },
        delete_after_archive: false,
    };

    let stats2 = archiver.archive(&policy2).await?;
    println!("  ✓ Archived {} rows to {}\n", stats2.archived_rows, daily_filename);

    // 示例 3: 按月份生成文件名
    println!("📆 Example 3: Generate monthly filename");
    let monthly_filename = policy1.generate_monthly_filename("/tmp/flux_archive", now);
    println!("  Monthly filename: {}", monthly_filename);
    // 示例: /tmp/flux_archive/device_metrics_2026-02.json

    // 示例 4: 按年份生成文件名
    println!("📊 Example 4: Generate yearly filename");
    let yearly_filename = policy1.generate_yearly_filename("/tmp/flux_archive", now);
    println!("  Yearly filename: {}", yearly_filename);
    // 示例: /tmp/flux_archive/device_metrics_2026.json

    // 示例 5: S3 归档（自动生成对象键）
    println!("\n☁️  Example 5: S3 archive with auto-generated key");
    let policy5 = ArchivePolicy {
        table_name: "device_metrics".to_string(),
        archive_older_than: Duration::days(365),
        destination: ArchiveDestination::S3 {
            bucket: "flux-iot-archive".to_string(),
            region: "us-west-2".to_string(),
            prefix: "metrics".to_string(),
        },
        delete_after_archive: false,
    };

    // 生成的 S3 键示例: metrics/device_metrics_20260222_180000.json
    println!("  S3 key will be auto-generated: metrics/device_metrics_<timestamp>.json");

    // 示例 6: 自定义文件名模板
    println!("\n🎨 Example 6: Custom filename patterns");
    println!("  Timestamp format: device_metrics_20260222_180000.json");
    println!("  Daily format:     device_metrics_2026-02-22.json");
    println!("  Monthly format:   device_metrics_2026-02.json");
    println!("  Yearly format:    device_metrics_2026.json");

    println!("\n✨ Dynamic archive example completed!");

    Ok(())
}
