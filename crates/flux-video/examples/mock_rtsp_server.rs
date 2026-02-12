// 模拟 RTSP 服务器 - 用于测试
// 生成模拟的 H.264 视频流

use std::time::Duration;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    let addr = "127.0.0.1:8554";
    let listener = TcpListener::bind(addr).await?;
    
    tracing::info!("Mock RTSP server listening on rtsp://{}/stream", addr);
    tracing::info!("Use this URL to test: rtsp://127.0.0.1:8554/stream");
    
    loop {
        let (mut socket, peer_addr) = listener.accept().await?;
        tracing::info!("New connection from: {}", peer_addr);
        
        tokio::spawn(async move {
            let mut buf = vec![0u8; 1024];
            
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        tracing::info!("Connection closed: {}", peer_addr);
                        break;
                    }
                    Ok(n) => {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        tracing::debug!("Received: {}", request);
                        
                        // 简单的 RTSP 响应
                        if request.contains("OPTIONS") {
                            let response = "RTSP/1.0 200 OK\r\n\
                                          CSeq: 1\r\n\
                                          Public: OPTIONS, DESCRIBE, SETUP, PLAY, TEARDOWN\r\n\
                                          \r\n";
                            let _ = socket.write_all(response.as_bytes()).await;
                        } else if request.contains("DESCRIBE") {
                            let sdp = format!(
                                "RTSP/1.0 200 OK\r\n\
                                 CSeq: 2\r\n\
                                 Content-Type: application/sdp\r\n\
                                 Content-Length: 200\r\n\
                                 \r\n\
                                 v=0\r\n\
                                 o=- 0 0 IN IP4 127.0.0.1\r\n\
                                 s=Mock Stream\r\n\
                                 t=0 0\r\n\
                                 m=video 0 RTP/AVP 96\r\n\
                                 a=rtpmap:96 H264/90000\r\n\
                                 a=control:track1\r\n"
                            );
                            let _ = socket.write_all(sdp.as_bytes()).await;
                        } else if request.contains("SETUP") {
                            let response = "RTSP/1.0 200 OK\r\n\
                                          CSeq: 3\r\n\
                                          Session: 12345678\r\n\
                                          Transport: RTP/AVP;unicast;client_port=50000-50001\r\n\
                                          \r\n";
                            let _ = socket.write_all(response.as_bytes()).await;
                        } else if request.contains("PLAY") {
                            let response = "RTSP/1.0 200 OK\r\n\
                                          CSeq: 4\r\n\
                                          Session: 12345678\r\n\
                                          \r\n";
                            let _ = socket.write_all(response.as_bytes()).await;
                            
                            // 开始发送模拟视频数据
                            tracing::info!("Starting to send mock video data...");
                            tokio::spawn(async move {
                                send_mock_video_stream().await;
                            });
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error reading from socket: {}", e);
                        break;
                    }
                }
            }
        });
    }
}

async fn send_mock_video_stream() {
    // 模拟发送 H.264 视频帧
    for i in 0..100 {
        tokio::time::sleep(Duration::from_millis(33)).await; // 30 fps
        tracing::debug!("Sending mock frame {}", i);
    }
}
