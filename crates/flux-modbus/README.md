# flux-modbus

Modbus 协议实现，支持 Modbus TCP 和 RTU。

## 功能特性

- ✅ Modbus TCP 客户端
- ✅ 读取保持寄存器
- ✅ 读取输入寄存器
- ✅ 读取线圈
- ✅ 读取离散输入
- ✅ 写入保持寄存器
- ✅ 写入线圈
- ✅ 统一协议接口

## 使用示例

```rust
use flux_modbus::{ModbusAdapter, ModbusConfig};
use flux_protocol::ProtocolClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ModbusConfig {
        host: "192.168.1.100".to_string(),
        port: 502,
        slave_id: 1,
        timeout_ms: 5000,
    };
    
    let mut client = ModbusAdapter::new(config);
    client.connect().await?;
    
    // 读取保持寄存器
    let value = client.read("holding/40001").await?;
    println!("Value: {}", value);
    
    // 写入保持寄存器
    client.write("holding/40001", serde_json::json!(100)).await?;
    
    Ok(())
}
```

## 地址格式

```
holding/40001      - 保持寄存器（可读写）
input/30001        - 输入寄存器（只读）
coil/00001         - 线圈（可读写）
discrete/10001     - 离散输入（只读）
```

## 许可证

MIT License
