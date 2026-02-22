use chrono::{Duration, Utc};
use flux_timeseries::{
    AggregationType, EventPoint, EventSeverity, LogLevel, LogPoint, MetricPoint, TimeSeriesQuery,
    TimeSeriesStore, TimescaleStore,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🚀 FLUX TimeSeries Example\n");

    // 连接 TimescaleDB
    let database_url = "postgresql://postgres:postgres@localhost:5432/flux_iot";
    let store = TimescaleStore::new(database_url).await?;
    println!("✅ Connected to TimescaleDB\n");

    // 1. 写入指标数据
    println!("📊 Writing metric data...");
    let metric = MetricPoint::new(
        "device_001".to_string(),
        "temperature".to_string(),
        25.5,
    )
    .with_unit("celsius".to_string())
    .with_tags(serde_json::json!({"location": "room_1", "floor": 1}));

    store.write_metric(&metric).await?;
    println!("  ✓ Metric written: temperature = 25.5°C\n");

    // 2. 批量写入指标
    println!("📊 Writing batch metrics...");
    let metrics = vec![
        MetricPoint::new("device_001".to_string(), "humidity".to_string(), 60.0)
            .with_unit("percent".to_string()),
        MetricPoint::new("device_001".to_string(), "pressure".to_string(), 1013.25)
            .with_unit("hPa".to_string()),
        MetricPoint::new("device_002".to_string(), "temperature".to_string(), 22.3)
            .with_unit("celsius".to_string()),
    ];

    store.write_metrics(&metrics).await?;
    println!("  ✓ {} metrics written\n", metrics.len());

    // 3. 写入日志
    println!("📝 Writing log data...");
    let log = LogPoint::new(
        "device_001".to_string(),
        LogLevel::Info,
        "Device started successfully".to_string(),
    )
    .with_source("system".to_string());

    store.write_log(&log).await?;
    println!("  ✓ Log written\n");

    // 4. 写入事件
    println!("🔔 Writing event data...");
    let event = EventPoint::new(
        "device_001".to_string(),
        "temperature_alert".to_string(),
        serde_json::json!({
            "threshold": 30.0,
            "current_value": 25.5,
            "message": "Temperature within normal range"
        }),
    )
    .with_severity(EventSeverity::Low);

    store.write_event(&event).await?;
    println!("  ✓ Event written\n");

    // 5. 查询最近的指标数据
    println!("🔍 Querying recent metrics...");
    let query = TimeSeriesQuery::new(Utc::now() - Duration::hours(1), Utc::now())
        .with_device("device_001".to_string())
        .with_metric("temperature".to_string())
        .with_limit(10);

    let results = store.query_metrics(&query).await?;
    println!("  ✓ Found {} metric points", results.len());
    for point in results.iter().take(3) {
        println!(
            "    - {} @ {}: {} {}",
            point.metric_name,
            point.timestamp.format("%H:%M:%S"),
            point.metric_value,
            point.unit.as_ref().unwrap_or(&"".to_string())
        );
    }
    println!();

    // 6. 查询聚合数据
    println!("📈 Querying aggregated data (5-minute average)...");
    let agg_query = TimeSeriesQuery::new(Utc::now() - Duration::hours(1), Utc::now())
        .with_device("device_001".to_string())
        .with_metric("temperature".to_string())
        .with_aggregation(AggregationType::Avg, 300); // 5 minutes

    let agg_results = store.query_aggregated(&agg_query).await?;
    println!("  ✓ Found {} aggregated points", agg_results.len());
    for point in agg_results.iter().take(3) {
        println!(
            "    - {} @ {}: avg = {:.2}",
            point.metric_name,
            point.bucket.format("%H:%M:%S"),
            point.value
        );
    }
    println!();

    println!("✨ Example completed successfully!");

    Ok(())
}
