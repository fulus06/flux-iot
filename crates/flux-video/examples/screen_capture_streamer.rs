// 屏幕捕获推流器 - 捕获屏幕并通过模拟 RTSP 推送
// 用于人工验证推流是否成功

use std::time::Duration;
use tokio::sync::mpsc;
use bytes::Bytes;

/// 模拟屏幕捕获（生成测试图案）
/// 实际应用中可以使用 scrap 或 screenshots crate 捕获真实屏幕
struct ScreenCapture {
    width: u32,
    height: u32,
    fps: u32,
    frame_count: u32,
}

impl ScreenCapture {
    fn new(width: u32, height: u32, fps: u32) -> Self {
        Self {
            width,
            height,
            fps,
            frame_count: 0,
        }
    }
    
    /// 生成测试图案帧（模拟屏幕捕获）
    fn capture_frame(&mut self) -> Vec<u8> {
        self.frame_count += 1;
        
        // 生成 H.264 格式的测试帧
        let mut data = Vec::new();
        
        // 每秒生成一个关键帧
        let is_keyframe = self.frame_count % self.fps == 0;
        
        if is_keyframe {
            // 关键帧：SPS + PPS + IDR
            data.extend_from_slice(&[0, 0, 0, 1]);
            data.push(0x67); // SPS
            data.extend_from_slice(&[0x42, 0x00, 0x1f, 0xe9, 0x02, 0xc1, 0x2c, 0x80]);
            
            data.extend_from_slice(&[0, 0, 0, 1]);
            data.push(0x68); // PPS
            data.extend_from_slice(&[0xce, 0x3c, 0x80]);
            
            data.extend_from_slice(&[0, 0, 0, 1]);
            data.push(0x65); // IDR
        } else {
            // P帧
            data.extend_from_slice(&[0, 0, 0, 1]);
            data.push(0x41); // P-frame
        }
        
        // 添加模拟的图像数据（包含帧号信息）
        let frame_marker = format!("Frame:{:06}", self.frame_count);
        data.extend_from_slice(frame_marker.as_bytes());
        data.extend_from_slice(&vec![0xAA; 500]); // 填充数据
        
        data
    }
}

/// 简单的 RTSP 推流服务器
async fn start_rtsp_push_server(
    port: u16,
    mut frame_rx: mpsc::Receiver<Bytes>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    
    tracing::info!("📡 RTSP 推流服务器启动: rtsp://{}/screen", addr);
    tracing::info!("   可以使用此 URL 连接到 flux-video");
    
    tokio::spawn(async move {
        if let Ok((mut socket, peer_addr)) = listener.accept().await {
            tracing::info!("✅ 客户端连接: {}", peer_addr);
            
            let mut buf = vec![0u8; 1024];
            
            // 处理 RTSP 握手
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        
                        if request.contains("OPTIONS") {
                            let response = "RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: OPTIONS, DESCRIBE, SETUP, PLAY\r\n\r\n";
                            let _ = socket.write_all(response.as_bytes()).await;
                        } else if request.contains("DESCRIBE") {
                            let sdp = format!(
                                "RTSP/1.0 200 OK\r\nCSeq: 2\r\nContent-Type: application/sdp\r\nContent-Length: 150\r\n\r\n\
                                v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=Screen Capture\r\nt=0 0\r\n\
                                m=video 0 RTP/AVP 96\r\na=rtpmap:96 H264/90000\r\n"
                            );
                            let _ = socket.write_all(sdp.as_bytes()).await;
                        } else if request.contains("SETUP") {
                            let response = "RTSP/1.0 200 OK\r\nCSeq: 3\r\nSession: 12345678\r\nTransport: RTP/AVP\r\n\r\n";
                            let _ = socket.write_all(response.as_bytes()).await;
                        } else if request.contains("PLAY") {
                            let response = "RTSP/1.0 200 OK\r\nCSeq: 4\r\nSession: 12345678\r\n\r\n";
                            let _ = socket.write_all(response.as_bytes()).await;
                            
                            tracing::info!("🎬 开始推送视频流...");
                            
                            // 开始发送视频帧
                            while let Some(frame) = frame_rx.recv().await {
                                // 模拟发送 RTP 包
                                tracing::trace!("发送帧: {} bytes", frame.len());
                            }
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("=== 屏幕捕获推流器 ===\n");
    
    // 配置
    let width = 1920;
    let height = 1080;
    let fps = 30;
    let duration_secs = 60; // 推流 60 秒
    
    println!("📺 配置:");
    println!("   分辨率: {}x{}", width, height);
    println!("   帧率: {} fps", fps);
    println!("   时长: {} 秒", duration_secs);
    println!();
    
    // 创建屏幕捕获器
    let mut capture = ScreenCapture::new(width, height, fps);
    
    // 创建帧通道
    let (frame_tx, frame_rx) = mpsc::channel(100);
    
    // 启动 RTSP 推流服务器
    start_rtsp_push_server(8554, frame_rx).await?;
    
    println!("📡 推流地址: rtsp://127.0.0.1:8554/screen");
    println!();
    println!("💡 使用方法:");
    println!("   1. 在另一个终端启动 flux-video 服务器:");
    println!("      cargo run --example video_server");
    println!();
    println!("   2. 创建流连接:");
    println!("      curl -X POST http://localhost:8080/api/video/streams \\");
    println!("        -H 'Content-Type: application/json' \\");
    println!("        -d '{{");
    println!("          \"stream_id\": \"screen_capture\",");
    println!("          \"protocol\": \"rtsp\",");
    println!("          \"url\": \"rtsp://127.0.0.1:8554/screen\"");
    println!("        }}'");
    println!();
    println!("   3. 在浏览器打开 Web 播放器:");
    println!("      http://localhost:8080/player.html?stream=screen_capture");
    println!();
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("🎬 开始捕获屏幕并推流...\n");
    
    // 捕获并推流
    let total_frames = fps * duration_secs;
    for i in 0..total_frames {
        let frame = capture.capture_frame();
        let frame_bytes = Bytes::from(frame);
        
        if frame_tx.send(frame_bytes).await.is_err() {
            tracing::warn!("客户端已断开");
            break;
        }
        
        // 每秒报告一次
        if i % fps == 0 {
            let seconds = i / fps;
            println!("⏱️  推流中... {} 秒 / {} 秒 (帧号: {})", seconds, duration_secs, i);
        }
        
        // 控制帧率
        tokio::time::sleep(Duration::from_millis(1000 / fps as u64)).await;
    }
    
    println!();
    println!("✅ 推流完成！");
    println!("   总帧数: {}", total_frames);
    println!("   总时长: {} 秒", duration_secs);
    
    Ok(())
}
