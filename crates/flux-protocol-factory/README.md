# flux-protocol-factory

统一协议工厂实现，提供从 URI 创建协议客户端的能力。

## 功能特性

- ✅ 统一的 URI 解析和协议路由
- ✅ 支持 Modbus、CoAP、OPC UA 协议
- ✅ 灵活的参数配置（通过 URI query string）
- ✅ Feature flags 支持可选协议
- ✅ 零循环依赖设计

## 使用示例

```rust
use flux_protocol_factory::{DefaultProtocolFactory, ProtocolFactory};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let factory = DefaultProtocolFactory::new();
    
    // 创建 Modbus 客户端
    let modbus_client = factory.from_uri(
        "modbus://192.168.1.100:502?slave_id=1&timeout_ms=5000"
    ).await?;
    
    // 创建 CoAP 客户端
    let coap_client = factory.from_uri(
        "coap://localhost:5683/sensors/temperature"
    ).await?;
    
    // 创建 OPC UA 客户端
    let opcua_client = factory.from_uri(
        "opcua://localhost:4840?username=admin&password=secret"
    ).await?;
    
    Ok(())
}
```

## 支持的 URI 格式

### Modbus

```
modbus://host:port?slave_id=<id>&timeout_ms=<ms>
```

参数：
- `slave_id`: 从站 ID（默认：1）
- `timeout_ms`: 超时时间（默认：5000）

示例：
```
modbus://192.168.1.100:502?slave_id=2&timeout_ms=3000
```

### CoAP

```
coap://host:port/path?timeout_ms=<ms>
```

参数：
- `timeout_ms`: 超时时间（默认：5000）

示例：
```
coap://localhost:5683/sensors/temperature?timeout_ms=2000
```

### OPC UA

```
opcua://host:port?security_policy=<policy>&security_mode=<mode>&username=<user>&password=<pass>
```

参数：
- `security_policy`: 安全策略（默认：None）
- `security_mode`: 安全模式（默认：None）
- `username`: 用户名（可选）
- `password`: 密码（可选）

示例：
```
opcua://localhost:4840?security_policy=None&username=admin
```

## Features

默认启用所有协议：

```toml
[dependencies]
flux-protocol-factory = "0.1"
```

仅启用特定协议：

```toml
[dependencies]
flux-protocol-factory = { version = "0.1", default-features = false, features = ["modbus"] }
```

可用 features：
- `modbus` - Modbus TCP 支持
- `coap` - CoAP 支持
- `opcua` - OPC UA 支持

## 架构设计

为避免循环依赖，协议工厂采用分层设计：

1. **flux-protocol**: 定义 `ProtocolFactory` trait 和 `ProtocolClient` trait
2. **flux-modbus/flux-coap/flux-opcua**: 实现各协议的 `ProtocolClient`
3. **flux-protocol-factory**: 实现 `ProtocolFactory` trait，依赖具体协议包

这种设计确保：
- 协议包只依赖 `flux-protocol`（trait 定义）
- 工厂包依赖协议包（具体实现）
- 无循环依赖

## 测试

```bash
cargo test -p flux-protocol-factory
```

所有测试：
- ✅ URI 解析测试（Modbus/CoAP/OpcUa）
- ✅ 客户端创建测试
- ✅ 默认参数测试
- ✅ 不支持协议错误处理测试
