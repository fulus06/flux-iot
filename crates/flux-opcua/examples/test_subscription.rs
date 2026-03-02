use flux_opcua::{OpcUaClientReal, OpcUaConfig};
use std::time::Duration;
use tokio::time::{interval, sleep};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== OPC UA 轮询监控测试 ===\n");
    println!("注意: 由于 opcua crate 订阅 API 复杂性，");
    println!("      使用定时轮询方式监控数据变化\n");

    // 创建配置
    let config = OpcUaConfig {
        endpoint_url: "opc.tcp://localhost:4840".to_string(),
        security_policy: "None".to_string(),
        security_mode: "None".to_string(),
        username: None,
        password: None,
    };

    // 创建客户端
    let mut client = OpcUaClientReal::new(config);

    // 连接到服务器
    println!("1. 连接到 OPC UA 服务器...");
    client.connect()?;
    println!("   ✅ 连接成功\n");

    // 要监控的节点
    let node_id = "ns=0;i=2258";  // Server/ServerStatus/CurrentTime
    println!("2. 开始轮询监控节点: {}", node_id);
    println!("   轮询间隔: 500ms");
    println!("   持续时间: 10秒\n");

    // 创建轮询间隔
    let mut poll_interval = interval(Duration::from_millis(500));
    let mut count = 0;
    let max_polls = 20;  // 10秒 / 500ms = 20次

    // 轮询监控
    println!("3. 轮询数据变化:");
    for i in 1..=max_polls {
        poll_interval.tick().await;
        
        match client.read_value(node_id) {
            Ok(value) => {
                println!("   [{}] 📊 {}", i, value);
                count += 1;
            }
            Err(e) => {
                println!("   [{}] ❌ 读取失败: {}", i, e);
            }
        }
    }

    println!("\n4. 轮询统计:");
    println!("   成功读取: {} 次", count);
    println!("   总轮询: {} 次", max_polls);
    println!("   成功率: {:.1}%", (count as f64 / max_polls as f64) * 100.0);

    // 断开连接
    println!("\n5. 断开连接...");
    client.disconnect()?;
    println!("   ✅ 已断开连接\n");

    println!("=== 测试完成 ===");
    println!("\n✅ OPC UA 轮询监控正常工作");
    println!("\n💡 提示:");
    println!("   - 轮询方式简单可靠");
    println!("   - 可根据需要调整轮询间隔");
    println!("   - 适合大多数监控场景");

    Ok(())
}
