// 完整流水线演示 - 使用模拟数据测试整个视频处理流程
// 演示：接入 -> 存储 -> 关键帧提取 -> 查询

use flux_video::{
    engine::VideoEngine,
    stream::RtspStream,
    snapshot::KeyframeExtractor,
    storage::StandaloneStorage,
    codec::H264Parser,
};
use std::sync::Arc;
use tokio::time::Duration;
use std::path::PathBuf;

/// 生成模拟的 H.264 视频帧
fn generate_mock_h264_frame(frame_number: u32, is_keyframe: bool) -> Vec<u8> {
    let mut data = Vec::new();
    
    if is_keyframe {
        // 关键帧：SPS + PPS + IDR
        // SPS
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.push(0x67); // SPS
        data.extend_from_slice(&[0x42, 0x00, 0x1f, 0xe9, 0x02, 0xc1, 0x2c, 0x80]);
        
        // PPS
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.push(0x68); // PPS
        data.extend_from_slice(&[0xce, 0x3c, 0x80]);
        
        // IDR
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.push(0x65); // IDR
        data.extend_from_slice(&[0x88, 0x84, 0x00, 0x10]);
        data.extend_from_slice(&vec![frame_number as u8; 200]); // 模拟数据
    } else {
        // 普通帧：P-frame
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.push(0x41); // P-frame
        data.extend_from_slice(&vec![frame_number as u8; 100]); // 模拟数据
    }
    
    data
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("=== FLUX Video 完整流水线演示 ===\n");
    
    // 1. 创建核心组件
    println!("📦 1. 初始化核心组件...");
    let engine = Arc::new(tokio::sync::RwLock::new(VideoEngine::new()));
    let mut storage = StandaloneStorage::new(PathBuf::from("./demo_data/storage"))?;
    let mut extractor = KeyframeExtractor::new(PathBuf::from("./demo_data/keyframes"))
        .with_interval(2); // 每 2 秒提取一次关键帧
    let mut parser = H264Parser::new();
    
    println!("   ✅ VideoEngine 已创建");
    println!("   ✅ StandaloneStorage 已创建");
    println!("   ✅ KeyframeExtractor 已创建");
    println!();
    
    // 2. 模拟 3 路摄像头流
    println!("📹 2. 模拟 3 路摄像头流...");
    let streams = vec![
        ("camera_001", "前门监控"),
        ("camera_002", "后门监控"),
        ("camera_003", "大厅监控"),
    ];
    
    for (stream_id, description) in &streams {
        let stream = RtspStream::new(
            stream_id.to_string(),
            format!("rtsp://mock.example.com/{}", stream_id),
        );
        
        let engine = engine.read().await;
        engine.publish_stream(stream_id.to_string(), Arc::new(stream))?;
        println!("   ✅ {} - {}", stream_id, description);
    }
    println!();
    
    // 3. 模拟视频流处理
    println!("🎬 3. 开始处理视频流（模拟 30 秒，30 fps）...");
    let total_frames = 30 * 30; // 30 秒 * 30 fps
    let mut keyframe_count = 0;
    let mut total_bytes = 0u64;
    
    for frame_num in 0..total_frames {
        // 每 30 帧（1秒）生成一个关键帧
        let is_keyframe = frame_num % 30 == 0;
        
        // 为每路摄像头生成帧
        for (stream_id, _) in &streams {
            let frame_data = generate_mock_h264_frame(frame_num, is_keyframe);
            let timestamp = chrono::Utc::now() + chrono::Duration::milliseconds(frame_num as i64 * 33);
            
            // 存储视频分片（每秒保存一次）
            if frame_num % 30 == 0 {
                let data = bytes::Bytes::from(frame_data.clone());
                total_bytes += data.len() as u64;
                storage.put_object(stream_id, timestamp, data).await?;
            }
            
            // 关键帧提取
            if is_keyframe {
                if let Some(keyframe) = extractor.process(stream_id, &frame_data, timestamp).await? {
                    keyframe_count += 1;
                    
                    if keyframe_count % 3 == 0 {
                        println!("   🎯 提取关键帧: {} (第 {} 帧, {} bytes)", 
                            stream_id, frame_num, keyframe.size());
                    }
                }
            }
            
            // 解析 H.264 NALU
            let nalus = parser.parse_annexb(&frame_data);
            if !nalus.is_empty() && frame_num == 0 {
                println!("   📊 解析 NALU: {} 个单元 (SPS={}, PPS={})", 
                    nalus.len(),
                    parser.sps().is_some(),
                    parser.pps().is_some()
                );
            }
        }
        
        // 模拟帧间隔（加速演示）
        if frame_num % 100 == 0 && frame_num > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    println!();
    
    // 4. 统计信息
    println!("📊 4. 处理统计...");
    println!("   总帧数: {} 帧", total_frames * streams.len() as u32);
    println!("   关键帧: {} 个", keyframe_count);
    println!("   存储数据: {:.2} MB", total_bytes as f64 / 1024.0 / 1024.0);
    println!("   流数量: {} 路", streams.len());
    println!();
    
    // 5. 查询测试
    println!("🔍 5. 测试数据查询...");
    let stream_id = "camera_001";
    let start = chrono::Utc::now() - chrono::Duration::seconds(35);
    let end = chrono::Utc::now();
    
    let objects = storage.list_objects(stream_id, start, end).await?;
    println!("   查询 {} 的录像: 找到 {} 个分片", stream_id, objects.len());
    
    if !objects.is_empty() {
        println!("   第一个分片:");
        println!("     - 时间: {}", objects[0].created_at.format("%H:%M:%S"));
        println!("     - 大小: {} bytes", objects[0].size);
        println!("     - 路径: {}", objects[0].path);
    }
    println!();
    
    // 6. 清理测试
    println!("🧹 6. 测试过期数据清理...");
    let before = chrono::Utc::now() - chrono::Duration::days(1);
    let deleted = storage.cleanup_expired(before).await?;
    println!("   清理了 {} 个过期对象", deleted);
    println!();
    
    // 7. 性能报告
    println!("⚡ 7. 性能报告...");
    println!("   处理速度: {} fps", total_frames * streams.len() as u32 / 30);
    println!("   平均每帧: {:.2} KB", total_bytes as f64 / (total_frames * streams.len() as u32) as f64 / 1024.0);
    println!("   关键帧比例: {:.1}%", keyframe_count as f64 / (total_frames * streams.len() as u32) as f64 * 100.0);
    println!();
    
    // 8. 验证引擎状态
    println!("✅ 8. 验证系统状态...");
    let engine = engine.read().await;
    let active_streams = engine.list_streams();
    println!("   活跃流: {:?}", active_streams);
    println!("   参数集: SPS={}, PPS={}", 
        parser.sps().is_some(), 
        parser.pps().is_some()
    );
    println!();
    
    println!("=== 演示完成！ ===");
    println!("\n💡 提示:");
    println!("   - 视频数据已保存到: ./demo_data/storage/");
    println!("   - 关键帧已保存到: ./demo_data/keyframes/");
    println!("   - 可以使用 'ls -lh ./demo_data/' 查看生成的文件");
    
    Ok(())
}
