use flux_modbus::{ModbusAdapter, ModbusConfig};
use flux_protocol::ProtocolClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🔧 FLUX Modbus Example\n");

    // 配置 Modbus 连接
    let config = ModbusConfig {
        host: "127.0.0.1".to_string(),
        port: 502,
        slave_id: 1,
        timeout_ms: 5000,
    };

    println!("📡 Connecting to Modbus server at {}:{}...", config.host, config.port);
    
    let mut client = ModbusAdapter::new(config);
    
    // 注意：需要实际的 Modbus 服务器才能运行
    // 可以使用 modbus-server 或 pymodbus 启动测试服务器
    
    match client.connect().await {
        Ok(_) => {
            println!("✅ Connected successfully!\n");
            
            // 读取保持寄存器
            println!("📖 Reading holding register 40001...");
            match client.read("holding/40001").await {
                Ok(value) => println!("  Value: {}\n", value),
                Err(e) => println!("  Error: {}\n", e),
            }
            
            // 写入保持寄存器
            println!("✍️  Writing value 100 to holding register 40001...");
            match client.write("holding/40001", serde_json::json!(100)).await {
                Ok(_) => println!("  ✅ Write successful\n"),
                Err(e) => println!("  ❌ Error: {}\n", e),
            }
            
            // 读取线圈
            println!("📖 Reading coil 00001...");
            match client.read("coil/00001").await {
                Ok(value) => println!("  Value: {}\n", value),
                Err(e) => println!("  Error: {}\n", e),
            }
            
            client.disconnect().await?;
            println!("👋 Disconnected");
        }
        Err(e) => {
            println!("❌ Connection failed: {}", e);
            println!("\n💡 Tip: Start a Modbus server first:");
            println!("   pip install pymodbus");
            println!("   pymodbus.server --host 127.0.0.1 --port 502");
        }
    }

    Ok(())
}
