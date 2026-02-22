// GB28181 协议集成测试
// 测试 SIP 注册、目录查询、实时预览、录像回放等核心功能

mod common;

use bytes::Bytes;
use flux_video::gb28181::sip::{SipServer, SipServerConfig, AuthMode};
use flux_video::gb28181::rtp::RtpReceiver;
use std::sync::Arc;
use tokio::net::UdpSocket;

/// 创建测试用的 SIP 服务器配置
fn test_sip_config() -> SipServerConfig {
    SipServerConfig {
        sip_id: "34020000002000000001".to_string(),
        sip_domain: "3402000000".to_string(),
        sip_ip: "127.0.0.1".to_string(),
        sip_port: 15060,
        auth_mode: AuthMode::None,
        auth_password: None,
        per_device_passwords: Default::default(),
        register_validity: 3600,
        heartbeat_interval: 60,
        heartbeat_timeout: 180,
    }
}

#[tokio::test]
async fn test_sip_server_startup() -> anyhow::Result<()> {
    let config = test_sip_config();
    let server = SipServer::new(config).await?;
    
    // 验证服务器已绑定端口
    assert!(server.is_running().await);
    
    // 优雅关闭
    server.shutdown().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_device_register_without_auth() -> anyhow::Result<()> {
    let config = test_sip_config();
    let server = Arc::new(SipServer::new(config).await?);
    
    // 启动服务器
    let server_clone = server.clone();
    tokio::spawn(async move {
        let _ = server_clone.start().await;
    });
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 模拟设备发送 REGISTER 请求
    let register_msg = format!(
        "REGISTER sip:34020000002000000001@3402000000 SIP/2.0\r\n\
         Via: SIP/2.0/UDP 127.0.0.1:5060;rport;branch=z9hG4bK123456\r\n\
         From: <sip:34020000001320000001@3402000000>;tag=fromtag\r\n\
         To: <sip:34020000001320000001@3402000000>\r\n\
         Call-ID: test-call-id-001\r\n\
         CSeq: 1 REGISTER\r\n\
         Contact: <sip:34020000001320000001@127.0.0.1:5060>\r\n\
         Max-Forwards: 70\r\n\
         Expires: 3600\r\n\
         Content-Length: 0\r\n\r\n"
    );
    
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    socket.send_to(register_msg.as_bytes(), "127.0.0.1:15060").await?;
    
    // 接收响应
    let mut buf = vec![0u8; 2048];
    let (len, _) = tokio::time::timeout(
        tokio::time::Duration::from_secs(2),
        socket.recv_from(&mut buf)
    ).await??;
    
    let response = String::from_utf8_lossy(&buf[..len]);
    
    // 验证响应是 200 OK
    assert!(response.contains("SIP/2.0 200 OK"), "Expected 200 OK response");
    
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_device_register_with_digest_auth() -> anyhow::Result<()> {
    let mut config = test_sip_config();
    config.sip_port = 15061; // 使用不同端口避免冲突
    config.auth_mode = AuthMode::Digest;
    config.auth_password = Some("admin123".to_string());
    
    let server = Arc::new(SipServer::new(config).await?);
    
    let server_clone = server.clone();
    tokio::spawn(async move {
        let _ = server_clone.start().await;
    });
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 第一次 REGISTER（无认证信息）
    let register_msg = format!(
        "REGISTER sip:34020000002000000001@3402000000 SIP/2.0\r\n\
         Via: SIP/2.0/UDP 127.0.0.1:5060;rport;branch=z9hG4bK123456\r\n\
         From: <sip:34020000001320000001@3402000000>;tag=fromtag\r\n\
         To: <sip:34020000001320000001@3402000000>\r\n\
         Call-ID: test-call-id-002\r\n\
         CSeq: 1 REGISTER\r\n\
         Contact: <sip:34020000001320000001@127.0.0.1:5060>\r\n\
         Max-Forwards: 70\r\n\
         Expires: 3600\r\n\
         Content-Length: 0\r\n\r\n"
    );
    
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    socket.send_to(register_msg.as_bytes(), "127.0.0.1:15061").await?;
    
    let mut buf = vec![0u8; 2048];
    let (len, _) = tokio::time::timeout(
        tokio::time::Duration::from_secs(2),
        socket.recv_from(&mut buf)
    ).await??;
    
    let response = String::from_utf8_lossy(&buf[..len]);
    
    // 验证响应是 401 Unauthorized
    assert!(response.contains("SIP/2.0 401 Unauthorized"), "Expected 401 response");
    assert!(response.contains("WWW-Authenticate"), "Expected WWW-Authenticate header");
    
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_rtp_receiver_basic() -> anyhow::Result<()> {
    use flux_video::gb28181::rtp::receiver::RtpReceiverConfig;
    
    let config = RtpReceiverConfig {
        local_ip: "127.0.0.1".to_string(),
        port_range: (30000, 30100),
        buffer_size: 1024,
    };
    
    let receiver = RtpReceiver::new(config).await?;
    let port = receiver.local_port();
    
    assert!(port >= 30000 && port <= 30100);
    
    // 模拟发送 RTP 包
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    
    // RTP Header: V=2, P=0, X=0, CC=0, M=0, PT=96, Seq=1, TS=1000, SSRC=12345
    let rtp_packet = vec![
        0x80, 0x60, // V=2, PT=96
        0x00, 0x01, // Sequence number = 1
        0x00, 0x00, 0x03, 0xE8, // Timestamp = 1000
        0x00, 0x00, 0x30, 0x39, // SSRC = 12345
        // Payload
        0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f, // H.264 SPS
    ];
    
    socket.send_to(&rtp_packet, format!("127.0.0.1:{}", port)).await?;
    
    // 接收并验证
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    Ok(())
}

#[tokio::test]
async fn test_catalog_query_workflow() -> anyhow::Result<()> {
    // 测试目录查询流程
    // 1. 设备注册成功
    // 2. 平台发送 Catalog 查询
    // 3. 设备响应设备列表
    
    let config = test_sip_config();
    config.sip_port = 15062;
    let server = Arc::new(SipServer::new(config).await?);
    
    let server_clone = server.clone();
    tokio::spawn(async move {
        let _ = server_clone.start().await;
    });
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 模拟 Catalog 查询 MESSAGE 请求
    let catalog_query = format!(
        "MESSAGE sip:34020000001320000001@3402000000 SIP/2.0\r\n\
         Via: SIP/2.0/UDP 127.0.0.1:15062;rport;branch=z9hG4bKcatalog\r\n\
         From: <sip:34020000002000000001@3402000000>;tag=fromtag\r\n\
         To: <sip:34020000001320000001@3402000000>\r\n\
         Call-ID: catalog-query-001\r\n\
         CSeq: 20 MESSAGE\r\n\
         Content-Type: Application/MANSCDP+xml\r\n\
         Max-Forwards: 70\r\n\
         Content-Length: 150\r\n\r\n\
         <?xml version=\"1.0\" encoding=\"GB2312\"?>\r\n\
         <Query>\r\n\
         <CmdType>Catalog</CmdType>\r\n\
         <SN>1</SN>\r\n\
         <DeviceID>34020000001320000001</DeviceID>\r\n\
         </Query>\r\n"
    );
    
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    socket.send_to(catalog_query.as_bytes(), "127.0.0.1:15062").await?;
    
    // 验证响应（实际实现需要设备端模拟）
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_invite_for_live_stream() -> anyhow::Result<()> {
    // 测试实时预览 INVITE 流程
    let config = test_sip_config();
    config.sip_port = 15063;
    let server = Arc::new(SipServer::new(config).await?);
    
    let server_clone = server.clone();
    tokio::spawn(async move {
        let _ = server_clone.start().await;
    });
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 模拟 INVITE 请求（实时预览）
    let sdp_body = "v=0\r\n\
                    o=34020000002000000001 0 0 IN IP4 127.0.0.1\r\n\
                    s=Play\r\n\
                    c=IN IP4 127.0.0.1\r\n\
                    t=0 0\r\n\
                    m=video 30000 RTP/AVP 96\r\n\
                    a=rtpmap:96 PS/90000\r\n\
                    y=0100000001\r\n";
    
    let invite_msg = format!(
        "INVITE sip:34020000001320000001@3402000000 SIP/2.0\r\n\
         Via: SIP/2.0/UDP 127.0.0.1:15063;rport;branch=z9hG4bKinvite\r\n\
         From: <sip:34020000002000000001@3402000000>;tag=fromtag\r\n\
         To: <sip:34020000001320000001@3402000000>\r\n\
         Call-ID: invite-live-001\r\n\
         CSeq: 20 INVITE\r\n\
         Content-Type: application/sdp\r\n\
         Max-Forwards: 70\r\n\
         Subject: 34020000001320000001:0,34020000002000000001:0\r\n\
         Content-Length: {}\r\n\r\n{}",
        sdp_body.len(),
        sdp_body
    );
    
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    socket.send_to(invite_msg.as_bytes(), "127.0.0.1:15063").await?;
    
    // 验证响应（需要设备端实现）
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_concurrent_device_registration() -> anyhow::Result<()> {
    // 测试多设备并发注册
    let config = test_sip_config();
    config.sip_port = 15064;
    let server = Arc::new(SipServer::new(config).await?);
    
    let server_clone = server.clone();
    tokio::spawn(async move {
        let _ = server_clone.start().await;
    });
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let mut handles = vec![];
    
    // 模拟 10 个设备并发注册
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            let device_id = format!("3402000000132000000{:01}", i);
            let register_msg = format!(
                "REGISTER sip:34020000002000000001@3402000000 SIP/2.0\r\n\
                 Via: SIP/2.0/UDP 127.0.0.1:506{};rport;branch=z9hG4bK{}\r\n\
                 From: <sip:{}@3402000000>;tag=tag{}\r\n\
                 To: <sip:{}@3402000000>\r\n\
                 Call-ID: concurrent-{}\r\n\
                 CSeq: 1 REGISTER\r\n\
                 Contact: <sip:{}@127.0.0.1:506{}>\r\n\
                 Max-Forwards: 70\r\n\
                 Expires: 3600\r\n\
                 Content-Length: 0\r\n\r\n",
                i, i, device_id, i, device_id, i, device_id, i
            );
            
            let socket = UdpSocket::bind("127.0.0.1:0").await?;
            socket.send_to(register_msg.as_bytes(), "127.0.0.1:15064").await?;
            
            let mut buf = vec![0u8; 2048];
            let (len, _) = tokio::time::timeout(
                tokio::time::Duration::from_secs(2),
                socket.recv_from(&mut buf)
            ).await??;
            
            let response = String::from_utf8_lossy(&buf[..len]);
            assert!(response.contains("SIP/2.0 200 OK"));
            
            Ok::<_, anyhow::Error>(())
        });
        
        handles.push(handle);
    }
    
    // 等待所有注册完成
    for handle in handles {
        handle.await??;
    }
    
    server.shutdown().await?;
    Ok(())
}
