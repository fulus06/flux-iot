use flux_srt::{
    caller::SrtCaller,
    listener::SrtListener,
    socket::SrtSocketConfig,
};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
#[ignore] // 需要实际网络环境
async fn test_listener_caller_connection() {
    // 启动 Listener
    let listener = SrtListener::bind("127.0.0.1:19000".parse().unwrap())
        .await
        .unwrap();

    let mut accept_rx = listener.start().await.unwrap();

    // 启动 Caller
    let caller_handle = tokio::spawn(async move {
        let caller = SrtCaller::new("127.0.0.1:19000".parse().unwrap())
            .await
            .unwrap();

        caller.connect().await
    });

    // 等待连接
    let result = timeout(Duration::from_secs(5), async {
        let server_socket = accept_rx.recv().await.unwrap();
        let client_socket = caller_handle.await.unwrap().unwrap();

        (server_socket, client_socket)
    })
    .await;

    assert!(result.is_ok(), "Connection should succeed");
}

#[tokio::test]
async fn test_socket_config() {
    let config = SrtSocketConfig {
        mtu: 1500,
        max_flow_window_size: 8192,
        latency_ms: 120,
        keepalive_interval_ms: 1000,
        connection_timeout_ms: 5000,
    };

    assert_eq!(config.mtu, 1500);
    assert_eq!(config.latency_ms, 120);
}

#[test]
fn test_module_exports() {
    // 验证所有模块都可以导入
    use flux_srt::ack::*;
    use flux_srt::bandwidth::*;
    use flux_srt::buffer::*;
    use flux_srt::congestion::*;
    use flux_srt::handshake::*;
    use flux_srt::packet::*;
    use flux_srt::statistics::*;

    // 基本类型检查
    let _ = AckPacket::new(100);
    let _ = NakPacket::new(vec![1, 2, 3]);
    let _ = SendBuffer::new(100);
    let _ = ReceiveBuffer::new(1, 100);
    let _ = CongestionController::new(10, 100);
    let _ = BandwidthEstimator::new(10);
    let _ = HandshakePacket::default();
    let _ = SrtStatistics::new(1);
}
