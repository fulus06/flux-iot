use flux_rtspd::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tempfile::TempDir;

/// 测试 RTSP 客户端基本连接
#[tokio::test]
async fn test_rtsp_client_basic() {
    // 注意：这个测试需要一个真实的 RTSP 服务器
    // 在 CI 环境中可以跳过或使用 mock
    // 这里仅作为示例
}

/// 测试 RTP 包解析
#[tokio::test]
async fn test_rtp_packet_parsing() {
    use flux_rtspd::rtp_receiver::RtpPacket;
    use bytes::Bytes;
    
    // 构造一个 RTP 包
    let payload = Bytes::from(vec![0x65, 0x01, 0x02, 0x03]); // H264 IDR
    let packet = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 1000,
        ssrc: 0x12345678,
        payload,
    };
    
    assert_eq!(packet.version, 2);
    assert_eq!(packet.sequence_number, 1);
    assert_eq!(packet.timestamp, 1000);
}

/// 测试 H264 解包器
#[tokio::test]
async fn test_h264_depacketizer() {
    use flux_rtspd::h264_depacketizer::H264Depacketizer;
    use flux_rtspd::rtp_receiver::RtpPacket;
    use bytes::Bytes;
    
    let mut depacketizer = H264Depacketizer::new();
    
    // 单个 NALU
    let payload = Bytes::from(vec![0x65, 0x01, 0x02, 0x03]);
    let packet = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 1000,
        ssrc: 0x12345678,
        payload,
    };
    
    let nalus = depacketizer.process_rtp(packet).unwrap();
    assert_eq!(nalus.len(), 1);
    assert!(nalus[0].is_keyframe);
}

/// 测试 H265 解包器
#[tokio::test]
async fn test_h265_depacketizer() {
    use flux_rtspd::h265_depacketizer::H265Depacketizer;
    use flux_rtspd::rtp_receiver::RtpPacket;
    use bytes::Bytes;
    
    let mut depacketizer = H265Depacketizer::new();
    
    // 单个 NALU (NAL type 19 = IDR)
    let payload = Bytes::from(vec![0x26, 0x01, 0x01, 0x02, 0x03]);
    let packet = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 1000,
        ssrc: 0x12345678,
        payload,
    };
    
    let nalus = depacketizer.process_rtp(packet).unwrap();
    assert_eq!(nalus.len(), 1);
    assert!(nalus[0].is_keyframe);
}

/// 测试 AAC 解包器
#[tokio::test]
async fn test_aac_depacketizer() {
    use flux_rtspd::aac_depacketizer::AacDepacketizer;
    use flux_rtspd::rtp_receiver::RtpPacket;
    use bytes::{Bytes, BytesMut};
    
    let mut depacketizer = AacDepacketizer::new();
    
    // 构造 AAC RTP 包
    let mut payload = BytesMut::new();
    payload.extend_from_slice(&[0x00, 0x10]); // AU-headers-length = 16 bits
    payload.extend_from_slice(&[0x00, 0x50]); // AU-header: size=10
    payload.extend_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A]);
    
    let packet = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 97,
        sequence_number: 1,
        timestamp: 1000,
        ssrc: 0x12345678,
        payload: payload.freeze(),
    };
    
    let frames = depacketizer.process_rtp(packet).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].data.len(), 10);
}

/// 测试 RTCP 解析
#[tokio::test]
async fn test_rtcp_parsing() {
    use flux_rtspd::rtcp_receiver::RtcpReceiver;
    
    // 构造一个简单的 SR 包
    let data = vec![
        0x80, 200, 0x00, 0x06, // V=2, PT=200(SR), length=6
        0x12, 0x34, 0x56, 0x78, // SSRC
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // NTP timestamp
        0x00, 0x00, 0x10, 0x00, // RTP timestamp
        0x00, 0x00, 0x00, 0x64, // Packet count (100)
        0x00, 0x00, 0x27, 0x10, // Octet count (10000)
    ];
    
    let packets = RtcpReceiver::parse_rtcp_packet(&data).unwrap();
    assert_eq!(packets.len(), 1);
}

/// 测试 SDP 解析
#[tokio::test]
async fn test_sdp_parsing() {
    use flux_rtspd::sdp_parser::SdpParser;
    
    let sdp = r#"v=0
o=- 0 0 IN IP4 127.0.0.1
s=Test Stream
c=IN IP4 0.0.0.0
t=0 0
m=video 0 RTP/AVP 96
a=rtpmap:96 H264/90000
a=fmtp:96 packetization-mode=1
a=control:track1
"#;
    
    let session = SdpParser::parse(sdp).unwrap();
    assert_eq!(session.media_descriptions.len(), 1);
    
    let video_track = SdpParser::get_video_track(&session).unwrap();
    assert_eq!(video_track.media_type, "video");
    assert_eq!(video_track.control_url, Some("track1".to_string()));
}

/// 测试流管理器创建
#[tokio::test]
async fn test_stream_manager_creation() {
    use flux_rtspd::stream_manager::RtspStreamManager;
    use flux_media_core::storage::{filesystem::FileSystemStorage, StorageConfig};
    use flux_media_core::snapshot::SnapshotOrchestrator;
    use flux_rtspd::telemetry::TelemetryClient;
    use std::path::PathBuf;
    
    let temp_dir = TempDir::new().unwrap();
    let storage_config = StorageConfig {
        root_dir: temp_dir.path().to_path_buf(),
        retention_days: 7,
        segment_duration_secs: 60,
    };
    
    let storage = Arc::new(RwLock::new(FileSystemStorage::new(storage_config).unwrap()));
    let orchestrator = Arc::new(SnapshotOrchestrator::new(PathBuf::from(temp_dir.path())));
    let telemetry = TelemetryClient::new(None, 5000);
    
    let _manager = RtspStreamManager::new(
        storage,
        orchestrator,
        None,
        telemetry,
    );
    
    // 管理器创建成功
}

/// 测试 AAC 配置解析
#[tokio::test]
async fn test_aac_config_parsing() {
    use flux_rtspd::aac_depacketizer::AacDepacketizer;
    
    let fmtp = "profile-level-id=1;mode=AAC-hbr;sizelength=13;indexlength=3;config=1190";
    let config = AacDepacketizer::parse_config(fmtp).unwrap();
    
    assert_eq!(config.sample_rate, 48000);
    assert_eq!(config.channels, 2);
    assert_eq!(config.size_length, 13);
    assert_eq!(config.index_length, 3);
}

/// 测试 H264 FU-A 分片
#[tokio::test]
async fn test_h264_fu_a_fragmentation() {
    use flux_rtspd::h264_depacketizer::H264Depacketizer;
    use flux_rtspd::rtp_receiver::RtpPacket;
    use bytes::Bytes;
    
    let mut depacketizer = H264Depacketizer::new();
    
    // FU-A 开始包
    let payload1 = Bytes::from(vec![
        0x7C, // FU indicator (type 28)
        0x85, // FU header: S=1, E=0, Type=5
        0x01, 0x02, 0x03,
    ]);
    let packet1 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: false,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 2000,
        ssrc: 0x12345678,
        payload: payload1,
    };
    
    let nalus1 = depacketizer.process_rtp(packet1).unwrap();
    assert_eq!(nalus1.len(), 0); // 未完成
    
    // FU-A 结束包
    let payload2 = Bytes::from(vec![
        0x7C, // FU indicator
        0x45, // FU header: S=0, E=1, Type=5
        0x04, 0x05, 0x06,
    ]);
    let packet2 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 2,
        timestamp: 2000,
        ssrc: 0x12345678,
        payload: payload2,
    };
    
    let nalus2 = depacketizer.process_rtp(packet2).unwrap();
    assert_eq!(nalus2.len(), 1);
    assert!(nalus2[0].is_keyframe);
}

/// 测试 H265 FU 分片
#[tokio::test]
async fn test_h265_fu_fragmentation() {
    use flux_rtspd::h265_depacketizer::H265Depacketizer;
    use flux_rtspd::rtp_receiver::RtpPacket;
    use bytes::Bytes;
    
    let mut depacketizer = H265Depacketizer::new();
    
    // FU 开始包
    let payload1 = Bytes::from(vec![
        0x62, 0x01, // Payload header: Type=49 (FU)
        0x93,       // FU header: S=1, E=0, Type=19
        0x01, 0x02, 0x03,
    ]);
    let packet1 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: false,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 2000,
        ssrc: 0x12345678,
        payload: payload1,
    };
    
    let nalus1 = depacketizer.process_rtp(packet1).unwrap();
    assert_eq!(nalus1.len(), 0); // 未完成
    
    // FU 结束包
    let payload2 = Bytes::from(vec![
        0x62, 0x01, // Payload header
        0x53,       // FU header: S=0, E=1, Type=19
        0x04, 0x05, 0x06,
    ]);
    let packet2 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 2,
        timestamp: 2000,
        ssrc: 0x12345678,
        payload: payload2,
    };
    
    let nalus2 = depacketizer.process_rtp(packet2).unwrap();
    assert_eq!(nalus2.len(), 1);
    assert!(nalus2[0].is_keyframe);
}
