# flux-opcua

OPC UA (OPC Unified Architecture) 协议实现，用于工业自动化和智能制造。

## 功能特性

- ✅ OPC UA 客户端
- ✅ 节点读写
- ✅ 数据类型转换
- ✅ 统一协议接口
- ⏳ 数据订阅（计划中）
- ⏳ 节点浏览（计划中）
- ⏳ 历史数据访问（计划中）

## 使用示例

```rust
use flux_opcua::{OpcUaAdapter, OpcUaConfig};
use flux_protocol::ProtocolClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = OpcUaConfig {
        endpoint_url: "opc.tcp://localhost:4840".to_string(),
        security_policy: "None".to_string(),
        security_mode: "None".to_string(),
        username: None,
        password: None,
    };
    
    let mut client = OpcUaAdapter::new(config);
    client.connect().await?;
    
    // 读取节点
    let value = client.read("ns=2;s=Machine.Temperature").await?;
    println!("Temperature: {}", value);
    
    // 写入节点
    client.write("ns=2;s=Machine.SetPoint", serde_json::json!(75.0)).await?;
    
    Ok(())
}
```

## 地址格式

OPC UA 使用 NodeId 格式：

```
ns=2;s=Machine.Temperature    - 字符串标识符
ns=3;i=1001                   - 数值标识符
ns=4;g=550e8400-e29b-41d4-a716-446655440000  - GUID 标识符
```

## 支持的数据类型

- Boolean
- SByte, Byte
- Int16, UInt16
- Int32, UInt32
- Int64, UInt64
- Float, Double
- String

## 许可证

MIT License
