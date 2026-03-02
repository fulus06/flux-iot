// 简单的 OPC UA 客户端示例 - 验证 API 使用
use opcua::client::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simple OPC UA Client Test ===\n");

    // 创建客户端
    let mut client = ClientBuilder::new()
        .application_name("Simple Test Client")
        .application_uri("urn:SimpleTestClient")
        .create_sample_keypair(true)
        .trust_server_certs(true)
        .session_retry_limit(3)
        .client()
        .ok_or("Failed to create client")?;

    println!("1. Client created");

    // 创建端点描述
    let endpoint: EndpointDescription = (
        "opc.tcp://localhost:4840",
        "None",
        MessageSecurityMode::None,
        UserTokenPolicy::anonymous(),
    ).into();

    println!("2. Connecting to opc.tcp://localhost:4840");

    // 连接到端点
    let session = client.connect_to_endpoint(endpoint, IdentityToken::Anonymous)?;

    println!("3. Connected! Session type: Arc<RwLock<Session>>");

    // 读取服务器时间节点
    let node_id = NodeId::new(0, 2258);
    let nodes_to_read = vec![node_id.into()];

    println!("4. Reading node ns=0;i=2258 (Server/ServerStatus/CurrentTime)");

    // 获取 session 的读锁
    let session_lock = session.read();
    let results = session_lock.read(&nodes_to_read, TimestampsToReturn::Both, 1.0)?;

    if let Some(result) = results.first() {
        println!("5. Read result:");
        println!("   Status: {:?}", result.status);
        if let Some(ref value) = result.value {
            println!("   Value: {:?}", value);
        }
        if let Some(ref ts) = result.server_timestamp {
            println!("   Server timestamp: {:?}", ts);
        }
    }

    println!("\n6. Disconnecting...");
    session_lock.disconnect();

    println!("7. Disconnected successfully");

    Ok(())
}
