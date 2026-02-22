# flux-protocol

协议抽象层，为 Modbus、CoAP、OPC UA 等多种协议提供统一的接口。

## 功能特性

- ✅ **协议无关** - 统一的 `ProtocolClient` trait
- ✅ **统一地址** - 支持 URI 格式的协议地址
- ✅ **类型安全** - Rust 类型系统保证
- ✅ **异步支持** - 基于 Tokio 异步运行时

## 支持的协议

| 协议 | 状态 | 应用场景 |
|------|------|---------|
| Modbus | ⏳ 开发中 | 简单工业设备 |
| CoAP | ⏳ 开发中 | 资源受限设备 |
| OPC UA | ⏳ 开发中 | 复杂工业系统 |

## 使用示例

```rust
use flux_protocol::{ProtocolFactory, ProtocolClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 从 URI 创建客户端
    let mut client = ProtocolFactory::from_uri(
        "modbus://192.168.1.100:502"
    ).await?;
    
    // 连接设备
    client.connect().await?;
    
    // 读取数据
    let value = client.read("holding/40001").await?;
    println!("Value: {}", value);
    
    // 写入数据
    client.write("holding/40001", serde_json::json!(100)).await?;
    
    // 订阅数据变化
    let handle = client.subscribe("holding/40001", Box::new(|value| {
        println!("Changed: {}", value);
    })).await?;
    
    Ok(())
}
```

## URI 格式

```
modbus://192.168.1.100:502/holding/40001
coap://[::1]:5683/sensors/temperature
opcua://localhost:4840/ns=2;s=Machine.Temperature
```

## 架构

```
ProtocolClient (trait)
    ↓
┌─────────────┬─────────────┬─────────────┐
│   Modbus    │    CoAP     │   OPC UA    │
│   Adapter   │   Adapter   │   Adapter   │
└─────────────┴─────────────┴─────────────┘
```

## 许可证

MIT License
