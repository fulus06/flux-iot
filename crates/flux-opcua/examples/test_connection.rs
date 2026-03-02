use flux_opcua::{OpcUaClient, OpcUaConfig};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== FLUX IOT OPC UA 客户端测试 ===\n");

    // 配置 OPC UA 客户端
    let config = OpcUaConfig {
        endpoint_url: "opc.tcp://localhost:4840".to_string(),
        security_policy: "None".to_string(),
        security_mode: "None".to_string(),
        username: None,
        password: None,
    };

    println!("1. 创建 OPC UA 客户端");
    println!("   端点: {}\n", config.endpoint_url);

    let mut client = OpcUaClient::new(config);

    // 连接到服务器
    println!("2. 连接到 OPC UA 服务器...");
    match client.connect().await {
        Ok(_) => println!("   ✅ 连接成功\n"),
        Err(e) => {
            println!("   ❌ 连接失败: {}\n", e);
            println!("提示：请确保 OPC UA 服务器正在运行：");
            println!("  docker run -d -p 4840:4840 --name flux-opcua-test open62541/open62541\n");
            return Err(e);
        }
    }

    // 检查连接状态
    println!("3. 检查连接状态");
    if client.is_connected() {
        println!("   ✅ 客户端已连接\n");
    } else {
        println!("   ❌ 客户端未连接\n");
        return Ok(());
    }

    // 读取节点值
    println!("4. 读取节点值");
    println!("   节点 ID: ns=0;i=2258 (Server/ServerStatus/CurrentTime)");
    
    match client.read_value("ns=0;i=2258").await {
        Ok(value) => {
            println!("   ✅ 读取成功:");
            println!("   {}\n", serde_json::to_string_pretty(&value)?);
        }
        Err(e) => {
            println!("   ⚠️  读取失败: {}", e);
            println!("   注意: 当前为简化实现，需要真实 OPC UA 服务器支持\n");
        }
    }

    // 写入节点值（示例）
    println!("5. 写入节点值（示例）");
    println!("   节点 ID: ns=2;s=TestValue");
    
    let test_value = serde_json::json!({
        "value": 42,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    match client.write_value("ns=2;s=TestValue", test_value).await {
        Ok(_) => println!("   ✅ 写入成功\n"),
        Err(e) => {
            println!("   ⚠️  写入失败: {}", e);
            println!("   注意: 当前为简化实现，需要真实 OPC UA 服务器支持\n");
        }
    }

    // 断开连接
    println!("6. 断开连接");
    client.disconnect().await?;
    println!("   ✅ 已断开连接\n");

    println!("=== 测试完成 ===\n");
    println!("说明：");
    println!("- 当前实现为简化版本");
    println!("- 完整功能需要配置真实的 OPC UA 服务器");
    println!("- 参考文档: docs/OPCUA_IMPLEMENTATION_GUIDE.md\n");

    Ok(())
}
