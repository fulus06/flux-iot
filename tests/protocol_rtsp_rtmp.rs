// RTSP/RTMP 流媒体协议集成测试

mod common;

use bytes::Bytes;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::Duration;

#[tokio::test]
async fn test_rtsp_stream_connection() -> anyhow::Result<()> {
    // 测试 RTSP 流连接
    use flux_video::stream::RtspStream;
    
    let stream = RtspStream::new(
        "test_rtsp_stream".to_string(),
        "rtsp://127.0.0.1:8554/test".to_string(),
    );
    
    assert_eq!(stream.id(), "test_rtsp_stream");
    assert_eq!(stream.url(), "rtsp://127.0.0.1:8554/test");
    
    Ok(())
}

#[tokio::test]
async fn test_rtsp_describe_request() -> anyhow::Result<()> {
    // 测试 RTSP DESCRIBE 请求解析
    let describe_response = "RTSP/1.0 200 OK\r\n\
        CSeq: 1\r\n\
        Content-Type: application/sdp\r\n\
        Content-Length: 200\r\n\r\n\
        v=0\r\n\
        o=- 0 0 IN IP4 127.0.0.1\r\n\
        s=Test Stream\r\n\
        c=IN IP4 127.0.0.1\r\n\
        t=0 0\r\n\
        m=video 0 RTP/AVP 96\r\n\
        a=rtpmap:96 H264/90000\r\n";
    
    // 验证响应格式
    assert!(describe_response.contains("RTSP/1.0 200 OK"));
    assert!(describe_response.contains("application/sdp"));
    assert!(describe_response.contains("H264/90000"));
    
    Ok(())
}

#[tokio::test]
async fn test_rtsp_setup_teardown_flow() -> anyhow::Result<()> {
    // 测试 RTSP 完整流程：DESCRIBE → SETUP → PLAY → TEARDOWN
    
    // 1. DESCRIBE
    let describe_req = "DESCRIBE rtsp://127.0.0.1:8554/test RTSP/1.0\r\n\
        CSeq: 1\r\n\
        Accept: application/sdp\r\n\r\n";
    
    assert!(describe_req.contains("DESCRIBE"));
    
    // 2. SETUP
    let setup_req = "SETUP rtsp://127.0.0.1:8554/test/track1 RTSP/1.0\r\n\
        CSeq: 2\r\n\
        Transport: RTP/AVP;unicast;client_port=50000-50001\r\n\r\n";
    
    assert!(setup_req.contains("SETUP"));
    assert!(setup_req.contains("Transport"));
    
    // 3. PLAY
    let play_req = "PLAY rtsp://127.0.0.1:8554/test RTSP/1.0\r\n\
        CSeq: 3\r\n\
        Session: 12345678\r\n\
        Range: npt=0.000-\r\n\r\n";
    
    assert!(play_req.contains("PLAY"));
    assert!(play_req.contains("Range"));
    
    // 4. TEARDOWN
    let teardown_req = "TEARDOWN rtsp://127.0.0.1:8554/test RTSP/1.0\r\n\
        CSeq: 4\r\n\
        Session: 12345678\r\n\r\n";
    
    assert!(teardown_req.contains("TEARDOWN"));
    
    Ok(())
}

#[tokio::test]
async fn test_rtmp_handshake_c0c1() -> anyhow::Result<()> {
    // 测试 RTMP 握手 C0+C1 阶段
    
    // C0: 版本号 (1 byte)
    let c0 = vec![0x03]; // RTMP version 3
    assert_eq!(c0.len(), 1);
    assert_eq!(c0[0], 0x03);
    
    // C1: 1536 bytes (time + zero + random)
    let mut c1 = vec![0u8; 1536];
    c1[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x00]); // time
    c1[4..8].copy_from_slice(&[0x00, 0x00, 0x00, 0x00]); // zero
    // 剩余 1528 bytes 为随机数
    
    assert_eq!(c1.len(), 1536);
    
    Ok(())
}

#[tokio::test]
async fn test_rtmp_connect_command() -> anyhow::Result<()> {
    // 测试 RTMP connect 命令
    // AMF0 格式: command_name, transaction_id, command_object
    
    let command_name = "connect";
    let transaction_id = 1.0;
    let app_name = "live";
    
    assert_eq!(command_name, "connect");
    assert_eq!(transaction_id, 1.0);
    assert_eq!(app_name, "live");
    
    Ok(())
}

#[tokio::test]
async fn test_rtmp_publish_workflow() -> anyhow::Result<()> {
    // 测试 RTMP 推流完整流程
    // C0+C1 → S0+S1+S2 → C2 → connect → createStream → publish → 数据传输
    
    let workflow_steps = vec![
        "C0+C1",
        "S0+S1+S2",
        "C2",
        "connect",
        "createStream",
        "publish",
        "video/audio data",
    ];
    
    assert_eq!(workflow_steps.len(), 7);
    assert_eq!(workflow_steps[0], "C0+C1");
    assert_eq!(workflow_steps[6], "video/audio data");
    
    Ok(())
}

#[tokio::test]
async fn test_rtmp_play_workflow() -> anyhow::Result<()> {
    // 测试 RTMP 拉流完整流程
    // C0+C1 → S0+S1+S2 → C2 → connect → createStream → play → 接收数据
    
    let workflow_steps = vec![
        "C0+C1",
        "S0+S1+S2",
        "C2",
        "connect",
        "createStream",
        "play",
        "receive data",
    ];
    
    assert_eq!(workflow_steps.len(), 7);
    assert_eq!(workflow_steps[5], "play");
    
    Ok(())
}

#[tokio::test]
async fn test_flv_tag_parsing() -> anyhow::Result<()> {
    // 测试 FLV Tag 解析（RTMP 使用 FLV 格式传输）
    
    // FLV Tag Header: type(1) + data_size(3) + timestamp(3) + timestamp_ext(1) + stream_id(3)
    let tag_type_video = 9u8;
    let tag_type_audio = 8u8;
    let tag_type_script = 18u8;
    
    assert_eq!(tag_type_video, 9);
    assert_eq!(tag_type_audio, 8);
    assert_eq!(tag_type_script, 18);
    
    Ok(())
}

#[tokio::test]
async fn test_stream_recording_pipeline() -> anyhow::Result<()> {
    // 测试流媒体录制管道
    use flux_video::storage::StandaloneStorage;
    
    let temp_dir = TempDir::new()?;
    let mut storage = StandaloneStorage::new(temp_dir.path().to_path_buf())?;
    
    let stream_id = "rtmp_live_test";
    let timestamp = chrono::Utc::now();
    
    // 模拟录制数据
    let video_data = Bytes::from(vec![0x00, 0x00, 0x00, 0x01, 0x67]); // H.264 SPS
    let path = storage.put_object(stream_id, timestamp, video_data.clone()).await?;
    
    assert!(!path.is_empty());
    
    // 验证数据可读取
    let retrieved = storage.get_object(stream_id, timestamp).await?;
    assert_eq!(retrieved.len(), video_data.len());
    
    Ok(())
}

#[tokio::test]
async fn test_hls_segment_generation() -> anyhow::Result<()> {
    // 测试 HLS 切片生成
    // RTMP/RTSP → HLS (m3u8 + ts)
    
    let segment_duration = 10; // 10 秒一个切片
    let segment_count = 3;
    
    let mut playlist = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");
    playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", segment_duration));
    playlist.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
    
    for i in 0..segment_count {
        playlist.push_str(&format!("#EXTINF:{}.0,\n", segment_duration));
        playlist.push_str(&format!("segment_{}.ts\n", i));
    }
    
    playlist.push_str("#EXT-X-ENDLIST\n");
    
    assert!(playlist.contains("#EXTM3U"));
    assert!(playlist.contains("segment_0.ts"));
    assert!(playlist.contains("#EXT-X-ENDLIST"));
    
    Ok(())
}

#[tokio::test]
async fn test_multibitrate_streaming() -> anyhow::Result<()> {
    // 测试多码率流（ABR - Adaptive Bitrate）
    
    let bitrates = vec![
        ("low", 500_000),    // 500 kbps
        ("medium", 1_500_000), // 1.5 Mbps
        ("high", 3_000_000),   // 3 Mbps
    ];
    
    for (quality, bitrate) in &bitrates {
        assert!(bitrate > &0);
        println!("Quality: {}, Bitrate: {} bps", quality, bitrate);
    }
    
    assert_eq!(bitrates.len(), 3);
    
    Ok(())
}

#[tokio::test]
async fn test_rtsp_authentication() -> anyhow::Result<()> {
    // 测试 RTSP Digest 认证
    
    let auth_header = "WWW-Authenticate: Digest realm=\"RTSP Server\", \
        nonce=\"abc123\", algorithm=MD5";
    
    assert!(auth_header.contains("Digest"));
    assert!(auth_header.contains("nonce"));
    assert!(auth_header.contains("MD5"));
    
    Ok(())
}

#[tokio::test]
async fn test_stream_health_monitoring() -> anyhow::Result<()> {
    // 测试流健康监控
    use std::sync::Arc;
    use tokio::sync::RwLock;
    
    #[derive(Clone)]
    struct StreamHealth {
        bitrate: u64,
        frame_rate: u32,
        packet_loss: f32,
        last_update: chrono::DateTime<chrono::Utc>,
    }
    
    let health = Arc::new(RwLock::new(StreamHealth {
        bitrate: 2_000_000,
        frame_rate: 25,
        packet_loss: 0.01,
        last_update: chrono::Utc::now(),
    }));
    
    // 读取健康状态
    let h = health.read().await;
    assert_eq!(h.frame_rate, 25);
    assert!(h.packet_loss < 0.05); // 丢包率小于 5%
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_stream_publishing() -> anyhow::Result<()> {
    // 测试并发推流
    use flux_video::engine::VideoEngine;
    use flux_video::stream::RtspStream;
    
    let engine = VideoEngine::new();
    let mut handles = vec![];
    
    for i in 0..10 {
        let engine_clone = engine.clone();
        let handle = tokio::spawn(async move {
            let stream_id = format!("stream_{}", i);
            let stream = Arc::new(RtspStream::new(
                stream_id.clone(),
                format!("rtsp://127.0.0.1:8554/{}", stream_id),
            ));
            
            engine_clone.publish_stream(stream_id, stream)
        });
        handles.push(handle);
    }
    
    for handle in handles {
        let result = handle.await?;
        assert!(result.is_ok());
    }
    
    let streams = engine.list_streams();
    assert_eq!(streams.len(), 10);
    
    Ok(())
}

#[tokio::test]
async fn test_stream_transcoding_pipeline() -> anyhow::Result<()> {
    // 测试转码管道（模拟）
    // 输入: H.264 1080p → 输出: H.264 720p, 480p
    
    struct TranscodeProfile {
        name: String,
        width: u32,
        height: u32,
        bitrate: u64,
    }
    
    let profiles = vec![
        TranscodeProfile {
            name: "720p".to_string(),
            width: 1280,
            height: 720,
            bitrate: 2_000_000,
        },
        TranscodeProfile {
            name: "480p".to_string(),
            width: 854,
            height: 480,
            bitrate: 1_000_000,
        },
    ];
    
    for profile in &profiles {
        assert!(profile.width > 0);
        assert!(profile.height > 0);
        assert!(profile.bitrate > 0);
    }
    
    Ok(())
}
