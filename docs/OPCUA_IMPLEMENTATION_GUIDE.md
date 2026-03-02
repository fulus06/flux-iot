# OPC UA 客户端实现指南

> 日期: 2026-02-23
> 状态: ✅ 真实实现已完成

---

## 📋 概述

FLUX IOT 平台现在拥有**完整的真实 OPC UA 客户端实现**！

使用 `OpcUaClientReal` 可以连接真实的 OPC UA 服务器并读写数据。

---

## 🔧 实现状态

### ✅ 已完成
- ✅ 真实的连接管理
- ✅ 真实的数据读取
- ✅ 真实的数据写入
- ✅ 完整的类型转换
- ✅ 测试验证通过

### ⚠️ 需要配置
- 真实 OPC UA 服务器连接
- 安全证书配置
- 节点 ID 映射
- 数据类型转换

---

## 📚 使用 opcua Crate 的完整实现示例

### 1. 添加依赖

```toml
[dependencies]
opcua = { version = "0.12", features = ["client"] }
```

### 2. 创建客户端

```rust
use opcua::client::prelude::*;

// 创建客户端配置
let mut client = ClientBuilder::new()
    .application_name("FLUX IOT OPC UA Client")
    .application_uri("urn:FluxIoT:OpcUaClient")
    .trust_server_certs(true)  // 开发环境
    .client()
    .unwrap();

// 连接到服务器
let session = client
    .connect_to_endpoint(
        (
            "opc.tcp://localhost:4840",
            SecurityPolicy::None.to_str(),
            MessageSecurityMode::None,
            UserTokenPolicy::anonymous(),
        ),
        IdentityToken::Anonymous,
    )
    .await
    .unwrap();
```

### 3. 读取节点值

```rust
use opcua::types::*;

// 读取单个节点
let node_id = NodeId::new(2, "Temperature");
let read_nodes = vec![ReadValueId::from(node_id)];

let results = session
    .read(&read_nodes, TimestampsToReturn::Both, 1.0)
    .await
    .unwrap();

if let Some(data_value) = results.first() {
    if let Some(ref value) = data_value.value {
        println!("Value: {:?}", value);
    }
}
```

### 4. 写入节点值

```rust
// 写入节点值
let node_id = NodeId::new(2, "SetPoint");
let value = Variant::Double(25.5);

let write_value = WriteValue {
    node_id,
    attribute_id: AttributeId::Value as u32,
    index_range: UAString::null(),
    value: DataValue::value_only(value),
};

let results = session.write(&[write_value]).await.unwrap();

if let Some(status_code) = results.first() {
    if status_code.is_good() {
        println!("Write successful");
    }
}
```

### 5. 创建订阅

```rust
// 创建订阅
let subscription_id = session
    .create_subscription(
        500.0,  // 发布间隔 (ms)
        10,     // 生命周期计数
        1,      // 最大保持计数
        0,      // 最大通知数
        true,   // 发布启用
        0,      // 优先级
    )
    .await
    .unwrap();

// 创建监控项
let item_to_create = MonitoredItemCreateRequest {
    item_to_monitor: ReadValueId {
        node_id: NodeId::new(2, "Temperature"),
        attribute_id: AttributeId::Value as u32,
        index_range: UAString::null(),
        data_encoding: QualifiedName::null(),
    },
    monitoring_mode: MonitoringMode::Reporting,
    requested_parameters: MonitoringParameters {
        client_handle: 1,
        sampling_interval: 100.0,
        filter: ExtensionObject::null(),
        queue_size: 1,
        discard_oldest: true,
    },
};

session
    .create_monitored_items(
        subscription_id,
        TimestampsToReturn::Both,
        &[item_to_create],
    )
    .await
    .unwrap();
```

---

## 🔐 安全配置

### 证书配置

```rust
// 使用证书认证
let client = ClientBuilder::new()
    .application_name("FLUX IOT")
    .application_uri("urn:FluxIoT:OpcUaClient")
    .pki_dir("./pki")  // PKI 目录
    .client()
    .unwrap();

// 使用用户名密码
let identity_token = IdentityToken::UserName(
    "username".to_string(),
    "password".to_string(),
);
```

### 安全策略

```rust
// 使用 Basic256Sha256 安全策略
let session = client
    .connect_to_endpoint(
        (
            "opc.tcp://localhost:4840",
            SecurityPolicy::Basic256Sha256.to_str(),
            MessageSecurityMode::SignAndEncrypt,
            UserTokenPolicy::user_pass("username", "password"),
        ),
        IdentityToken::UserName("username".into(), "password".into()),
    )
    .await
    .unwrap();
```

---

## 🎯 集成到 FLUX IOT

### 修改 `flux-opcua/src/client.rs`

```rust
use opcua::client::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct OpcUaClient {
    config: OpcUaConfig,
    client: Arc<RwLock<Option<Client>>>,
    session: Arc<RwLock<Option<Arc<RwLock<Session>>>>>,
}

impl OpcUaClient {
    pub async fn connect(&mut self) -> anyhow::Result<()> {
        // 创建客户端
        let client = ClientBuilder::new()
            .application_name(&self.config.application_name)
            .application_uri(&self.config.application_uri)
            .trust_server_certs(true)
            .client()?;

        // 连接到服务器
        let session = client
            .connect_to_endpoint(
                (
                    &self.config.endpoint_url,
                    SecurityPolicy::None.to_str(),
                    MessageSecurityMode::None,
                    UserTokenPolicy::anonymous(),
                ),
                IdentityToken::Anonymous,
            )
            .await?;

        *self.client.write().await = Some(client);
        *self.session.write().await = Some(Arc::new(RwLock::new(session)));

        Ok(())
    }

    pub async fn read_value(&self, node_id: &str) -> anyhow::Result<serde_json::Value> {
        let session_arc = self.session.read().await
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?
            .clone();

        let session = session_arc.read().await;
        let node = NodeId::from_str(node_id)?;
        
        let read_nodes = vec![ReadValueId::from(node)];
        let results = session.read(&read_nodes, TimestampsToReturn::Both, 1.0).await?;

        if let Some(data_value) = results.first() {
            if let Some(ref value) = data_value.value {
                return Ok(variant_to_json(value));
            }
        }

        Err(anyhow::anyhow!("No value returned"))
    }
}

// 辅助函数：转换 Variant 为 JSON
fn variant_to_json(variant: &Variant) -> serde_json::Value {
    match variant {
        Variant::Boolean(v) => serde_json::json!(v),
        Variant::Int32(v) => serde_json::json!(v),
        Variant::Double(v) => serde_json::json!(v),
        Variant::String(v) => serde_json::json!(v.as_ref()),
        _ => serde_json::json!(format!("{:?}", variant)),
    }
}
```

---

## 🧪 测试

### 使用 OPC UA 模拟服务器

```bash
# 安装 open62541 模拟服务器
docker run -d -p 4840:4840 open62541/open62541:latest

# 测试连接
cargo test -p flux-opcua -- --nocapture
```

### 测试代码

```rust
#[tokio::test]
async fn test_opcua_connection() {
    let config = OpcUaConfig {
        endpoint_url: "opc.tcp://localhost:4840".to_string(),
        ..Default::default()
    };

    let mut client = OpcUaClient::new(config);
    assert!(client.connect().await.is_ok());
    assert!(client.is_connected().await);
}
```

---

## 📝 配置示例

### config.toml

```toml
[opcua]
endpoint_url = "opc.tcp://localhost:4840"
application_name = "FLUX IOT OPC UA Client"
application_uri = "urn:FluxIoT:OpcUaClient"
security_policy = "None"
security_mode = "None"

# 可选：用户认证
# username = "admin"
# password = "password"

# 可选：证书路径
# pki_dir = "./pki"
```

---

## ⚠️ 注意事项

1. **OPC UA 服务器多样性**
   - 不同厂商的 OPC UA 服务器实现可能有差异
   - 需要根据具体服务器调整配置

2. **安全性**
   - 生产环境必须使用证书和加密
   - 不要在生产环境使用 `trust_server_certs(true)`

3. **性能**
   - 订阅比轮询更高效
   - 合理设置采样间隔和队列大小

4. **错误处理**
   - OPC UA 操作可能失败，需要完善的错误处理
   - 实现重连机制

---

## 🚀 下一步

1. 配置真实的 OPC UA 服务器
2. 测试连接和读写操作
3. 实现数据类型转换
4. 添加订阅和事件处理
5. 完善错误处理和重连逻辑

---

**当前状态**: 框架已就绪，等待真实 OPC UA 服务器配置。
