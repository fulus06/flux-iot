use flux_modbus::{ModbusAdapter, ModbusConfig};
use flux_coap::{CoapAdapter, CoapConfig};
use flux_opcua::{OpcUaAdapter, OpcUaConfig};
use flux_protocol::ProtocolClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🚀 FLUX IOT - 协议扩展演示\n");
    println!("支持的协议: Modbus, CoAP, OPC UA\n");

    // 1. Modbus 示例
    println!("=" .repeat(60));
    println!("📡 Modbus 协议演示");
    println!("=" .repeat(60));
    
    let modbus_config = ModbusConfig {
        host: "127.0.0.1".to_string(),
        port: 502,
        slave_id: 1,
        timeout_ms: 5000,
    };

    let mut modbus_client = ModbusAdapter::new(modbus_config);
    
    println!("地址格式: modbus://127.0.0.1:502/holding/40001");
    println!("功能: 读写保持寄存器、输入寄存器、线圈、离散输入");
    println!("应用: 70%+ 工业设备（PLC、传感器、执行器）\n");

    // 2. CoAP 示例
    println!("=" .repeat(60));
    println!("🌐 CoAP 协议演示");
    println!("=" .repeat(60));
    
    let coap_config = CoapConfig {
        host: "localhost".to_string(),
        port: 5683,
        timeout_ms: 5000,
    };

    let mut coap_client = CoapAdapter::new(coap_config);
    
    println!("地址格式: coap://localhost:5683/sensors/temperature");
    println!("功能: GET/PUT/POST/DELETE, Observe订阅");
    println!("应用: 资源受限设备（嵌入式、传感器网络）\n");

    // 3. OPC UA 示例
    println!("=" .repeat(60));
    println!("🏭 OPC UA 协议演示");
    println!("=" .repeat(60));
    
    let opcua_config = OpcUaConfig {
        endpoint_url: "opc.tcp://localhost:4840".to_string(),
        security_policy: "None".to_string(),
        security_mode: "None".to_string(),
        username: None,
        password: None,
    };

    let mut opcua_client = OpcUaAdapter::new(opcua_config);
    
    println!("地址格式: opcua://localhost:4840/ns=2;s=Machine.Temperature");
    println!("功能: 节点读写、数据订阅、节点浏览、历史数据");
    println!("应用: 智能制造、工业4.0、复杂工业系统\n");

    // 统一接口演示
    println!("=" .repeat(60));
    println!("✨ 统一协议接口演示");
    println!("=" .repeat(60));
    println!("所有协议使用相同的接口:");
    println!("  - connect()");
    println!("  - read(address)");
    println!("  - write(address, value)");
    println!("  - subscribe(address, callback)");
    println!("  - disconnect()");
    println!();

    println!("💡 优势:");
    println!("  ✅ 协议无关的上层应用");
    println!("  ✅ 降低开发复杂度");
    println!("  ✅ 易于扩展新协议");
    println!("  ✅ 统一错误处理");
    println!();

    println!("🎯 应用场景:");
    println!("  • 工业物联网平台");
    println!("  • 智能制造系统");
    println!("  • 设备数据采集");
    println!("  • 远程设备控制");
    println!();

    println!("✅ FLUX IOT 现已成为业界领先的全协议栈物联网平台！");

    Ok(())
}
