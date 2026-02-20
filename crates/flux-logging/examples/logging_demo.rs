use flux_logging::{
    create_span, extract_trace_ids, init_tracer, LogAggregator, LogEntry, LogEntryBuilder,
    LogLevel, LogSampler, SamplingStrategy, TracerConfig,
};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("=== FLUX IOT 日志增强系统示例 ===\n");

    // 1. 初始化追踪器
    println!("1. 初始化追踪器");
    let config = TracerConfig::default();
    init_tracer(config).unwrap();
    println!("追踪器已初始化\n");

    // 2. 创建结构化日志
    println!("2. 创建结构化日志");
    let span = create_span("demo_operation");
    let (trace_id, span_id) = extract_trace_ids(&span);

    let log_entry = LogEntryBuilder::new(LogLevel::Info, "用户登录成功".to_string())
        .target("flux_iot::auth".to_string())
        .trace_id(trace_id.clone())
        .span_id(span_id.clone())
        .field("user_id".to_string(), serde_json::json!("user-123"))
        .field("ip".to_string(), serde_json::json!("192.168.1.100"))
        .field("duration_ms".to_string(), serde_json::json!(45))
        .build();

    println!("结构化日志（JSON）：");
    println!("{}\n", log_entry.to_json_pretty().unwrap());

    // 3. 日志采样
    println!("3. 日志采样演示");

    // 3.1 按比例采样
    println!("3.1 按比例采样（50%）");
    let sampler = LogSampler::new(SamplingStrategy::Ratio(0.5));
    let mut sampled = 0;
    for i in 0..100 {
        if sampler.should_sample(LogLevel::Info).await {
            sampled += 1;
        }
    }
    println!("  100 条日志中采样了 {} 条\n", sampled);

    // 3.2 按级别采样
    println!("3.2 按级别采样");
    let level_sampler = LogSampler::new(SamplingStrategy::ByLevel {
        trace: 0.0,
        debug: 0.1,
        info: 0.5,
        warn: 1.0,
        error: 1.0,
    });

    println!("  Trace 级别: {}", level_sampler.should_sample(LogLevel::Trace).await);
    println!("  Debug 级别: {}", level_sampler.should_sample(LogLevel::Debug).await);
    println!("  Info 级别: {}", level_sampler.should_sample(LogLevel::Info).await);
    println!("  Warn 级别: {}", level_sampler.should_sample(LogLevel::Warn).await);
    println!("  Error 级别: {}\n", level_sampler.should_sample(LogLevel::Error).await);

    // 3.3 速率限制
    println!("3.3 速率限制（每秒最多 10 条）");
    let rate_sampler = LogSampler::new(SamplingStrategy::RateLimit(10));
    let mut allowed = 0;
    for _ in 0..20 {
        if rate_sampler.should_sample(LogLevel::Info).await {
            allowed += 1;
        }
    }
    println!("  20 条日志中允许了 {} 条\n", allowed);

    // 4. 日志聚合
    println!("4. 日志聚合演示");
    let temp_file = NamedTempFile::new().unwrap();
    let log_path = temp_file.path().to_path_buf();

    let aggregator = Arc::new(
        LogAggregator::new(5, 10).with_output_path(log_path.clone())
    );

    println!("  添加 10 条日志到聚合器...");
    for i in 0..10 {
        let span = create_span(format!("operation_{}", i));
        let (trace_id, span_id) = extract_trace_ids(&span);

        let entry = LogEntry::new(
            LogLevel::Info,
            format!("处理请求 {}", i),
            "flux_iot::api".to_string(),
        )
        .with_trace_id(trace_id)
        .with_span_id(span_id)
        .with_field("request_id".to_string(), serde_json::json!(format!("req-{}", i)));

        aggregator.add_log(entry).await;
    }

    // 等待自动刷新
    sleep(Duration::from_millis(100)).await;

    println!("  缓冲区大小: {}", aggregator.buffer_size().await);
    println!("  日志已写入文件: {:?}\n", log_path);

    // 读取并显示文件内容
    let content = tokio::fs::read_to_string(&log_path).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();
    println!("  文件中的日志条数: {}", lines.len());
    if !lines.is_empty() {
        println!("  第一条日志:");
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(lines[0]) {
            println!("  {}", serde_json::to_string_pretty(&parsed).unwrap());
        }
    }

    // 5. trace_id 关联
    println!("\n5. trace_id 关联演示");
    let parent_span = create_span("parent_operation");
    let (parent_trace_id, parent_span_id) = extract_trace_ids(&parent_span);

    println!("  父操作:");
    println!("    trace_id: {}", parent_trace_id);
    println!("    span_id: {}", parent_span_id);

    let child_span = create_span("child_operation");
    let (child_trace_id, child_span_id) = extract_trace_ids(&child_span);

    println!("  子操作:");
    println!("    trace_id: {}", child_trace_id);
    println!("    span_id: {}", child_span_id);

    let parent_log = LogEntry::new(
        LogLevel::Info,
        "开始处理订单".to_string(),
        "flux_iot::order".to_string(),
    )
    .with_trace_id(parent_trace_id.clone())
    .with_span_id(parent_span_id);

    let child_log = LogEntry::new(
        LogLevel::Info,
        "查询库存".to_string(),
        "flux_iot::inventory".to_string(),
    )
    .with_trace_id(child_trace_id)
    .with_span_id(child_span_id)
    .with_field("parent_span_id".to_string(), serde_json::json!(parent_span_id));

    println!("\n  父操作日志:");
    println!("{}", parent_log.to_json_pretty().unwrap());

    println!("\n  子操作日志（包含 parent_span_id）:");
    println!("{}", child_log.to_json_pretty().unwrap());

    println!("\n=== 示例完成 ===");
}
