use flux_rtspd::rtsp_client::{RtspClient, TransportMode};

/// 测试 TCP 传输模式设置
#[tokio::test]
async fn test_tcp_transport_mode() {
    let mut client = RtspClient::new("rtsp://localhost:554/stream".to_string());
    client.set_transport_mode(TransportMode::Tcp);
    
    // 验证传输模式已设置（通过 SETUP 请求验证）
    // 注意：这个测试需要真实的 RTSP 服务器，这里仅作为示例
}

/// 测试 Interleaved 数据包结构
#[test]
fn test_interleaved_packet() {
    use flux_rtspd::rtsp_client::InterleavedPacket;
    use bytes::Bytes;
    
    let packet = InterleavedPacket {
        channel: 0,
        data: Bytes::from(vec![0x01, 0x02, 0x03]),
    };
    
    assert_eq!(packet.channel, 0);
    assert_eq!(packet.data.len(), 3);
}

/// 测试 TransportMode 枚举
#[test]
fn test_transport_mode_enum() {
    use flux_rtspd::rtsp_client::TransportMode;
    
    let udp_mode = TransportMode::Udp;
    let tcp_mode = TransportMode::Tcp;
    
    assert_ne!(udp_mode, tcp_mode);
    assert_eq!(udp_mode, TransportMode::Udp);
    assert_eq!(tcp_mode, TransportMode::Tcp);
}
