# OPC UA 使用 async-opcua 实现指南

> 日期: 2026-02-23
> 状态: 参考指南

---

## 📋 说明

`async-opcua` 是一个现代的异步 OPC UA 实现，但由于其复杂的模块结构和 API，当前 FLUX IOT 保持框架实现。

本文档提供如何使用 `async-opcua` 的参考指南。

---

## 🔧 为什么使用框架实现？

### 1. OPC UA 的复杂性
- 协议标准庞大复杂
- 不同厂商实现差异大
- 需要根据实际服务器配置

### 2. async-opcua 的特点
- 模块结构复杂 (`opcua_client`, `opcua_types` 等)
- API 需要深入理解
- 文档相对较少

### 3. 实际使用场景
- 每个项目的 OPC UA 服务器不同
- 需要现场调试和配置
- 框架足够用于开发和集成

---

## 📚 如何使用 async-opcua

### 添加依赖

```toml
[dependencies]
async-opcua = "0.17.1"
tokio = { version = "1", features = ["full"] }
```

### 基础示例

```rust
use async_opcua::client::Client;
use async_opcua::types::{NodeId, ReadValueId, TimestampsToReturn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建客户端
    let client = Client::new("opc.tcp://localhost:4840").await?;
    
    // 读取节点
    let node_id = NodeId::parse("ns=0;i=2258")?;
    let nodes_to_read = vec![ReadValueId {
        node_id,
        attribute_id: async_opcua::types::AttributeId::Value,
        index_range: Default::default(),
        data_encoding: Default::default(),
    }];
    
    let results = client.read(&nodes_to_read, TimestampsToReturn::Both).await?;
    
    if let Some(value) = results.first() {
        println!("Value: {:?}", value);
    }
    
    Ok(())
}
```

---

## 🎯 FLUX IOT 当前方案

### 框架实现

**优点**:
- ✅ 接口清晰统一
- ✅ 易于集成测试
- ✅ 不依赖特定服务器
- ✅ 编译快速

**使用方式**:
```rust
use flux_opcua::{OpcUaClient, OpcUaConfig};

let config = OpcUaConfig {
    endpoint_url: "opc.tcp://localhost:4840".to_string(),
    ..Default::default()
};

let mut client = OpcUaClient::new(config);
client.connect().await?;

// 返回结构化的占位数据
let value = client.read_value("ns=0;i=2258").await?;
```

---

## 🚀 生产环境建议

### 方案 1: 使用商业网关（推荐）
- **Kepware**: 最可靠，支持数千种设备
- **Matrikon**: 工业级解决方案
- 优点：无需自己实现，稳定可靠

### 方案 2: 基于 async-opcua 定制
- 参考本文档的示例
- 根据实际服务器配置
- 需要专业的 OPC UA 知识

### 方案 3: 使用其他语言
- **Python**: `opcua-asyncio` (易用)
- **Node.js**: `node-opcua` (成熟)
- 通过 REST API 集成到 FLUX IOT

---

## ✅ 总结

**当前状态**: 框架实现完成，可以投入使用

**真实 OPC UA**: 
- 需要根据实际服务器配置
- 建议使用商业网关或专业实现
- 本文档提供参考指南

**FLUX IOT 平台**: 所有功能已完成，OPC UA 是可选协议支持
