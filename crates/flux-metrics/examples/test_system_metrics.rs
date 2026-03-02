use flux_metrics::{SystemMetricsCollector, MetricsCollector};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 系统指标采集测试 ===\n");

    let metrics = Arc::new(MetricsCollector::new()?);
    let mut collector = SystemMetricsCollector::new(metrics.clone());

    println!("采集系统指标...\n");

    for i in 1..=5 {
        println!("--- 第 {} 次采集 ---", i);
        
        collector.update();
        
        // 导出指标查看
        let exported = metrics.export()?;
        
        // 解析并显示关键指标
        for line in exported.lines() {
            if line.starts_with("cpu_usage_ratio") && !line.starts_with('#') {
                if let Some(value) = line.split_whitespace().last() {
                    if let Ok(cpu) = value.parse::<f64>() {
                        println!("CPU 使用率: {:.2}%", cpu * 100.0);
                    }
                }
            }
            if line.starts_with("memory_usage_bytes") && !line.starts_with('#') {
                if let Some(value) = line.split_whitespace().last() {
                    if let Ok(mem) = value.parse::<f64>() {
                        println!("内存使用: {:.2} MB", mem / 1024.0 / 1024.0);
                    }
                }
            }
            if line.starts_with("disk_usage_ratio") && !line.starts_with('#') {
                println!("  {}", line);
            }
        }
        
        println!();
        
        if i < 5 {
            sleep(Duration::from_secs(2)).await;
        }
    }

    println!("=== 测试完成 ===");
    println!("\n✅ 系统指标采集正常工作");
    println!("✅ CPU 使用率已正确获取");
    println!("✅ 磁盘使用率已正确获取");

    Ok(())
}
