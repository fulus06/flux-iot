# flux-coap

CoAP (Constrained Application Protocol) 协议实现，用于资源受限的物联网设备。

## 功能特性

- ✅ CoAP 客户端
- ✅ GET/PUT/POST/DELETE 方法
- ✅ 统一协议接口
- ⏳ Observe 订阅（计划中）
- ⏳ 资源发现（计划中）

## 使用示例

```rust
use flux_coap::{CoapAdapter, CoapConfig};
use flux_protocol::ProtocolClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = CoapConfig {
        host: "localhost".to_string(),
        port: 5683,
        timeout_ms: 5000,
    };
    
    let mut client = CoapAdapter::new(config);
    client.connect().await?;
    
    // GET 请求
    let value = client.read("/sensors/temperature").await?;
    println!("Temperature: {}", value);
    
    // PUT 请求
    client.write("/actuators/led", serde_json::json!({"state": "on"})).await?;
    
    Ok(())
}
```

## 地址格式

```
coap://localhost:5683/sensors/temperature
coap://[::1]:5683/actuators/led
```

## CoAP 方法

- **GET**: 读取资源
- **PUT**: 更新资源
- **POST**: 创建资源
- **DELETE**: 删除资源

## 许可证

MIT License
