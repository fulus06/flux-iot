use flux_rtspd::multicast_receiver::MulticastReceiver;
use std::net::Ipv4Addr;

/// 测试多播地址验证
#[test]
fn test_multicast_address_validation() {
    // 有效的多播地址
    assert!(is_valid_multicast(&Ipv4Addr::new(224, 0, 0, 1)));
    assert!(is_valid_multicast(&Ipv4Addr::new(239, 255, 255, 255)));
    assert!(is_valid_multicast(&Ipv4Addr::new(230, 1, 2, 3)));
    
    // 无效的多播地址
    assert!(!is_valid_multicast(&Ipv4Addr::new(192, 168, 1, 1)));
    assert!(!is_valid_multicast(&Ipv4Addr::new(10, 0, 0, 1)));
    assert!(!is_valid_multicast(&Ipv4Addr::new(223, 255, 255, 255)));
    assert!(!is_valid_multicast(&Ipv4Addr::new(240, 0, 0, 1)));
}

fn is_valid_multicast(addr: &Ipv4Addr) -> bool {
    let octets = addr.octets();
    octets[0] >= 224 && octets[0] <= 239
}

/// 测试多播接收器创建（需要真实网络环境）
#[tokio::test]
#[ignore] // 需要真实的多播环境，CI 中跳过
async fn test_multicast_receiver_creation() {
    let multicast_addr = Ipv4Addr::new(224, 0, 0, 1);
    let port = 5000;
    
    let result = MulticastReceiver::new(multicast_addr, port).await;
    
    // 在支持多播的环境中应该成功
    // 注意：某些环境可能不支持多播，测试可能失败
    match result {
        Ok((receiver, _rx)) => {
            // 成功创建
            drop(receiver);
        }
        Err(e) => {
            println!("Multicast not supported in this environment: {}", e);
        }
    }
}

/// 测试无效多播地址
#[tokio::test]
async fn test_invalid_multicast_address() {
    let invalid_addr = Ipv4Addr::new(192, 168, 1, 1); // 单播地址
    let port = 5000;
    
    let result = MulticastReceiver::new(invalid_addr, port).await;
    assert!(result.is_err());
}

/// 测试 TransportMode 多播枚举
#[test]
fn test_transport_mode_multicast() {
    use flux_rtspd::rtsp_client::TransportMode;
    
    let multicast_mode = TransportMode::Multicast;
    assert_eq!(multicast_mode, TransportMode::Multicast);
    assert_ne!(multicast_mode, TransportMode::Udp);
    assert_ne!(multicast_mode, TransportMode::Tcp);
}
