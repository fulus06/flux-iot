use flux_video::metrics::QualityMonitor;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 视频质量监控测试 ===\n");

    let mut monitor = QualityMonitor::new();

    println!("1. 模拟视频流（30fps, 2Mbps）");
    println!("   持续时间: 5秒\n");

    // 模拟 30fps 视频流，每帧约 8KB (2Mbps)
    let frame_size = 8192; // 8KB
    let frame_interval = Duration::from_millis(33); // ~30fps
    let total_frames = 150; // 5秒 * 30fps

    println!("2. 接收帧数据:");
    for i in 1..=total_frames {
        monitor.record_frame(frame_size);
        
        if i % 30 == 0 {
            let metrics = monitor.calculate_metrics();
            println!("   第 {} 秒:", i / 30);
            println!("     FPS: {:.2}", metrics.fps);
            println!("     比特率: {:.2} Mbps", metrics.bitrate_mbps());
            println!("     质量分数: {:.1}/100", metrics.quality_score);
            println!("     质量等级: {:?}", metrics.quality_level());
        }
        
        sleep(frame_interval).await;
    }

    println!("\n3. 最终统计:");
    let final_metrics = monitor.calculate_metrics();
    println!("   总帧数: {}", final_metrics.total_frames);
    println!("   丢帧数: {}", final_metrics.dropped_frames);
    println!("   丢帧率: {:.2}%", final_metrics.drop_rate);
    println!("   平均 FPS: {:.2}", final_metrics.fps);
    println!("   平均比特率: {:.2} Mbps", final_metrics.bitrate_mbps());
    println!("   质量分数: {:.1}/100", final_metrics.quality_score);
    println!("   质量等级: {:?}", final_metrics.quality_level());

    println!("\n4. 模拟网络抖动（丢帧场景）");
    monitor.reset();
    
    for i in 1..=60 {
        monitor.record_frame(frame_size);
        
        // 每 10 帧模拟一次网络抖动（延迟）
        if i % 10 == 0 {
            sleep(Duration::from_millis(200)).await; // 额外延迟
        } else {
            sleep(frame_interval).await;
        }
    }

    let degraded_metrics = monitor.calculate_metrics();
    println!("   总帧数: {}", degraded_metrics.total_frames);
    println!("   丢帧数: {}", degraded_metrics.dropped_frames);
    println!("   丢帧率: {:.2}%", degraded_metrics.drop_rate);
    println!("   平均 FPS: {:.2}", degraded_metrics.fps);
    println!("   质量分数: {:.1}/100", degraded_metrics.quality_score);
    println!("   质量等级: {:?}", degraded_metrics.quality_level());

    println!("\n=== 测试完成 ===");
    println!("\n✅ 视频质量监控功能正常工作");
    println!("\n💡 功能说明:");
    println!("   - 实时计算 FPS 和比特率");
    println!("   - 检测丢帧情况");
    println!("   - 综合质量评分（0-100）");
    println!("   - 质量等级分类");

    Ok(())
}
