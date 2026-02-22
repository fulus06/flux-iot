# 协议抽象层设计 - 统一协议接口

> **设计日期**: 2026-02-22  
> **版本**: v1.0.0

---

## 🎯 设计目标

为 Modbus、CoAP、OPC UA 等多种协议提供**统一的抽象接口**，实现：

1. ✅ **协议无关** - 上层应用不关心底层协议
2. ✅ **可扩展** - 轻松添加新协议
3. ✅ **类型安全** - Rust 类型系统保证
4. ✅ **异步支持** - 基于 Tokio 异步运行时

---

## 📊 协议对比

| 协议 | 应用场景 | 复杂度 | 数据模型 |
|------|---------|--------|---------|
| **Modbus** | 简单工业设备 | 低 | 寄存器 |
| **CoAP** | 资源受限设备 | 中 | RESTful |
| **OPC UA** | 复杂工业系统 | 高 | 对象模型 |

---

## 🏗️ 架构设计

### 分层架构

```
┌─────────────────────────────────────┐
│      应用层 (Application)            │
│  设备管理、数据采集、控制逻辑         │
└─────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────┐
│    协议抽象层 (Protocol Trait)       │
│  统一的 Read/Write/Subscribe 接口    │
└─────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────┐
│      协议适配器 (Adapters)           │
├─────────────┬─────────────┬─────────┤
│   Modbus    │    CoAP     │  OPC UA │
│   Adapter   │   Adapter   │ Adapter │
└─────────────┴─────────────┴─────────┘
                 ↓
┌─────────────────────────────────────┐
│      协议实现 (Implementations)      │
├─────────────┬─────────────┬─────────┤
│   Modbus    │    CoAP     │  OPC UA │
│   Client    │   Client    │ Client  │
└─────────────┴─────────────┴─────────┘
```

---

## 🔧 核心接口设计

### 1. 协议客户端 Trait

```rust
use async_trait::async_trait;
use serde_json::Value;

/// 统一协议客户端接口
#[async_trait]
pub trait ProtocolClient: Send + Sync {
    /// 连接设备
    async fn connect(&mut self) -> Result<()>;
    
    /// 断开连接
    async fn disconnect(&mut self) -> Result<()>;
    
    /// 读取数据
    async fn read(&self, address: &str) -> Result<Value>;
    
    /// 批量读取
    async fn read_multiple(&self, addresses: &[String]) -> Result<Vec<Value>>;
    
    /// 写入数据
    async fn write(&self, address: &str, value: Value) -> Result<()>;
    
    /// 批量写入
    async fn write_multiple(&self, data: &[(String, Value)]) -> Result<()>;
    
    /// 订阅数据变化
    async fn subscribe(
        &self,
        address: &str,
        callback: Box<dyn Fn(Value) + Send + Sync>,
    ) -> Result<SubscriptionHandle>;
    
    /// 取消订阅
    async fn unsubscribe(&self, handle: SubscriptionHandle) -> Result<()>;
    
    /// 获取协议类型
    fn protocol_type(&self) -> ProtocolType;
    
    /// 检查连接状态
    fn is_connected(&self) -> bool;
}
```

### 2. 协议类型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolType {
    Modbus,
    CoAP,
    OpcUa,
    Mqtt,
    Http,
}
```

### 3. 地址抽象

```rust
/// 统一地址格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolAddress {
    /// 协议类型
    pub protocol: ProtocolType,
    
    /// 设备地址
    pub device: String,
    
    /// 数据点地址（协议相关）
    pub address: String,
    
    /// 额外参数
    pub params: HashMap<String, String>,
}

impl ProtocolAddress {
    /// 从 URI 解析
    /// 示例:
    /// - modbus://192.168.1.100:502/holding/40001
    /// - coap://[::1]:5683/sensors/temperature
    /// - opcua://localhost:4840/ns=2;s=Machine.Temperature
    pub fn from_uri(uri: &str) -> Result<Self> {
        // 解析逻辑
    }
    
    /// 转换为 URI
    pub fn to_uri(&self) -> String {
        // 转换逻辑
    }
}
```

---

## 📋 协议适配器

### Modbus 适配器

```rust
pub struct ModbusAdapter {
    client: ModbusClient,
    config: ModbusConfig,
}

#[async_trait]
impl ProtocolClient for ModbusAdapter {
    async fn read(&self, address: &str) -> Result<Value> {
        // 解析地址: "holding/40001"
        let (register_type, addr) = parse_modbus_address(address)?;
        
        match register_type {
            RegisterType::Holding => {
                let value = self.client.read_holding_register(addr).await?;
                Ok(json!(value))
            }
            RegisterType::Input => {
                let value = self.client.read_input_register(addr).await?;
                Ok(json!(value))
            }
            // ...
        }
    }
    
    async fn write(&self, address: &str, value: Value) -> Result<()> {
        let (register_type, addr) = parse_modbus_address(address)?;
        let val = value.as_u64().ok_or("Invalid value")?;
        
        self.client.write_holding_register(addr, val as u16).await?;
        Ok(())
    }
    
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Modbus
    }
}
```

### CoAP 适配器

```rust
pub struct CoapAdapter {
    client: CoapClient,
    config: CoapConfig,
}

#[async_trait]
impl ProtocolClient for CoapAdapter {
    async fn read(&self, address: &str) -> Result<Value> {
        // 地址格式: "/sensors/temperature"
        let response = self.client.get(address).await?;
        let value: Value = serde_json::from_slice(&response.payload)?;
        Ok(value)
    }
    
    async fn write(&self, address: &str, value: Value) -> Result<()> {
        let payload = serde_json::to_vec(&value)?;
        self.client.put(address, payload).await?;
        Ok(())
    }
    
    async fn subscribe(
        &self,
        address: &str,
        callback: Box<dyn Fn(Value) + Send + Sync>,
    ) -> Result<SubscriptionHandle> {
        // CoAP Observe
        let handle = self.client.observe(address, move |data| {
            if let Ok(value) = serde_json::from_slice(&data) {
                callback(value);
            }
        }).await?;
        
        Ok(SubscriptionHandle::new(handle))
    }
    
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::CoAP
    }
}
```

### OPC UA 适配器

```rust
pub struct OpcUaAdapter {
    client: OpcUaClient,
    config: OpcUaConfig,
}

#[async_trait]
impl ProtocolClient for OpcUaAdapter {
    async fn read(&self, address: &str) -> Result<Value> {
        // 地址格式: "ns=2;s=Machine.Temperature"
        let node_id = NodeId::parse(address)?;
        let value = self.client.read_value(&node_id).await?;
        
        // 转换 OPC UA 数据类型到 JSON
        Ok(opcua_value_to_json(value))
    }
    
    async fn write(&self, address: &str, value: Value) -> Result<()> {
        let node_id = NodeId::parse(address)?;
        let opcua_value = json_to_opcua_value(value)?;
        
        self.client.write_value(&node_id, opcua_value).await?;
        Ok(())
    }
    
    async fn subscribe(
        &self,
        address: &str,
        callback: Box<dyn Fn(Value) + Send + Sync>,
    ) -> Result<SubscriptionHandle> {
        let node_id = NodeId::parse(address)?;
        
        let handle = self.client.subscribe(&node_id, move |value| {
            callback(opcua_value_to_json(value));
        }).await?;
        
        Ok(SubscriptionHandle::new(handle))
    }
    
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::OpcUa
    }
}
```

---

## 🎨 协议工厂

```rust
pub struct ProtocolFactory;

impl ProtocolFactory {
    /// 根据配置创建协议客户端
    pub async fn create(config: ProtocolConfig) -> Result<Box<dyn ProtocolClient>> {
        match config.protocol_type {
            ProtocolType::Modbus => {
                let adapter = ModbusAdapter::new(config.modbus_config?).await?;
                Ok(Box::new(adapter))
            }
            ProtocolType::CoAP => {
                let adapter = CoapAdapter::new(config.coap_config?).await?;
                Ok(Box::new(adapter))
            }
            ProtocolType::OpcUa => {
                let adapter = OpcUaAdapter::new(config.opcua_config?).await?;
                Ok(Box::new(adapter))
            }
            _ => Err(anyhow!("Unsupported protocol")),
        }
    }
    
    /// 从 URI 创建
    pub async fn from_uri(uri: &str) -> Result<Box<dyn ProtocolClient>> {
        let address = ProtocolAddress::from_uri(uri)?;
        let config = ProtocolConfig::from_address(&address)?;
        Self::create(config).await
    }
}
```

---

## 💡 使用示例

### 示例 1: 统一读取

```rust
// 读取 Modbus 设备
let modbus_client = ProtocolFactory::from_uri(
    "modbus://192.168.1.100:502/holding/40001"
).await?;

let value = modbus_client.read("holding/40001").await?;
println!("Modbus value: {}", value);

// 读取 CoAP 设备
let coap_client = ProtocolFactory::from_uri(
    "coap://[::1]:5683/sensors/temperature"
).await?;

let value = coap_client.read("/sensors/temperature").await?;
println!("CoAP value: {}", value);

// 读取 OPC UA 设备
let opcua_client = ProtocolFactory::from_uri(
    "opcua://localhost:4840/ns=2;s=Machine.Temperature"
).await?;

let value = opcua_client.read("ns=2;s=Machine.Temperature").await?;
println!("OPC UA value: {}", value);
```

### 示例 2: 协议无关的数据采集

```rust
async fn collect_data(
    client: &dyn ProtocolClient,
    addresses: &[String],
) -> Result<Vec<Value>> {
    // 不关心具体协议，统一接口
    client.read_multiple(addresses).await
}

// 使用
let modbus_data = collect_data(&modbus_client, &modbus_addresses).await?;
let coap_data = collect_data(&coap_client, &coap_addresses).await?;
let opcua_data = collect_data(&opcua_client, &opcua_addresses).await?;
```

### 示例 3: 统一订阅

```rust
async fn subscribe_all(clients: Vec<Box<dyn ProtocolClient>>) -> Result<()> {
    for client in clients {
        client.subscribe("data_point", Box::new(|value| {
            println!("Protocol: {:?}, Value: {}", 
                client.protocol_type(), value);
        })).await?;
    }
    Ok(())
}
```

---

## 📦 包结构

```
crates/
├── flux-protocol/              # 协议抽象层
│   ├── src/
│   │   ├── lib.rs
│   │   ├── client.rs          # ProtocolClient trait
│   │   ├── address.rs         # ProtocolAddress
│   │   ├── factory.rs         # ProtocolFactory
│   │   └── types.rs           # 公共类型
│   └── Cargo.toml
│
├── flux-modbus/                # Modbus 实现
│   ├── src/
│   │   ├── lib.rs
│   │   ├── client.rs          # Modbus 客户端
│   │   ├── adapter.rs         # Modbus 适配器
│   │   └── types.rs
│   └── Cargo.toml
│
├── flux-coap/                  # CoAP 实现
│   ├── src/
│   │   ├── lib.rs
│   │   ├── client.rs          # CoAP 客户端
│   │   ├── adapter.rs         # CoAP 适配器
│   │   └── types.rs
│   └── Cargo.toml
│
└── flux-opcua/                 # OPC UA 实现
    ├── src/
    │   ├── lib.rs
    │   ├── client.rs          # OPC UA 客户端
    │   ├── adapter.rs         # OPC UA 适配器
    │   └── types.rs
    └── Cargo.toml
```

---

## ✅ 设计优势

### 1. 协议无关
- ✅ 上层应用不关心底层协议
- ✅ 轻松切换协议
- ✅ 支持多协议混合

### 2. 可扩展
- ✅ 添加新协议只需实现 `ProtocolClient` trait
- ✅ 不影响现有代码

### 3. 类型安全
- ✅ Rust 类型系统保证
- ✅ 编译时检查

### 4. 异步支持
- ✅ 基于 Tokio
- ✅ 高性能

---

## 🎯 实施计划

### 阶段 1: 协议抽象层（1天）
- ✅ 定义 `ProtocolClient` trait
- ✅ 实现 `ProtocolAddress`
- ✅ 实现 `ProtocolFactory`

### 阶段 2: Modbus 实现（2天）
- ✅ Modbus 客户端
- ✅ Modbus 适配器
- ✅ 测试

### 阶段 3: CoAP 实现（2天）
- ✅ CoAP 客户端
- ✅ CoAP 适配器
- ✅ 测试

### 阶段 4: OPC UA 实现（3天）
- ✅ OPC UA 客户端
- ✅ OPC UA 适配器
- ✅ 测试

### 阶段 5: 集成测试（1天）
- ✅ 端到端测试
- ✅ 性能测试
- ✅ 文档

**总工期**: 9天（约 2周）

---

**维护者**: FLUX IOT Team  
**设计日期**: 2026-02-22  
**状态**: ✅ **设计完成，准备实施**
