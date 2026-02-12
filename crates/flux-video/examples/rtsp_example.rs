// RTSP 流示例
use flux_video::stream::{RtspStream, StreamState};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    // 创建 RTSP 流
    let mut stream = RtspStream::new(
        "camera_001".to_string(),
        "rtsp://localhost:8554/stream".to_string(),
    );
    
    println!("Starting RTSP stream...");
    stream.start().await?;
    
    // 等待连接
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 检查状态
    let state = stream.state().await;
    println!("Stream state: {:?}", state);
    
    // 接收媒体包（示例：接收 10 个包）
    let mut count = 0;
    while count < 10 {
        if let Some(packet) = stream.recv().await {
            println!(
                "Received packet: type={:?}, size={} bytes, is_key={}",
                packet.media_type,
                packet.data.len(),
                packet.is_key
            );
            count += 1;
        } else {
            break;
        }
    }
    
    // 停止流
    println!("Stopping stream...");
    stream.stop().await?;
    
    println!("Done!");
    Ok(())
}
