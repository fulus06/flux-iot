# flux-mqtt

MQTT Broker 实现，支持 MQTT v3.1.1 和 v5.0 协议。

## 功能特性

### 已实现 ✅

#### 核心协议
- ✅ **MQTT v3.1.1 支持** - 完整的 MQTT 3.1.1 协议实现
- ✅ **MQTT v5.0 支持** - 完整的 MQTT 5.0 协议实现
- ✅ **QoS 0/1 支持** - At Most Once 和 At Least Once
- ✅ **Retained 消息** - 保存主题最后一条消息
- ✅ **主题通配符** - 支持 `+` 和 `#` 通配符

#### 安全和权限
- ✅ **认证集成** - 集成 Authenticator trait
- ✅ **访问控制 ACL** - 主题级别权限控制
- ✅ **TLS/SSL 配置** - MQTTS 配置支持

#### 高级特性
- ✅ **EventBus 集成** - 双向消息转发
- ✅ **监控指标** - Prometheus 格式指标导出
- ✅ **持久化支持** - 会话和离线消息持久化（可选）
- ✅ **高性能** - 基于 ntex 异步框架

### 待实现 ⏳

- ⏳ **QoS 2 支持** - Exactly Once（当前降级为 QoS 1）
- ⏳ **持久化会话** - 会话和离线消息持久化
- ⏳ **Will 消息** - 遗嘱消息
- ⏳ **访问控制 ACL** - 主题级别权限控制
- ⏳ **WebSocket 支持** - MQTT over WebSocket
- ⏳ **监控指标** - Prometheus 指标暴露

## 快速开始

### 基本使用

```rust
use flux_core::bus::EventBus;
use flux_core::traits::auth::Authenticator;
use flux_mqtt::start_broker;
use std::sync::Arc;

// 创建 EventBus
let event_bus = Arc::new(EventBus::new());

// 创建认证器
let authenticator = Arc::new(YourAuthenticator);

// 启动 MQTT broker (端口 1883)
start_broker(event_bus, authenticator);
```

### 使用 TLS

```rust
use flux_mqtt::{start_broker_with_tls, tls::TlsConfig};

// 配置 TLS
let tls_config = TlsConfig::new(
    "certs/server.crt".to_string(),
    "certs/server.key".to_string(),
);

// 启动 MQTT (1883) 和 MQTTS (8883)
start_broker_with_tls(event_bus, authenticator, Some(tls_config));
```

### 使用访问控制 ACL

```rust
use flux_mqtt::{
    acl::{AclAction, AclPermission, AclRule, MqttAcl},
    manager::MqttManager,
};

// 创建 ACL 规则
let rules = vec![
    AclRule {
        client_id: Some("sensor_*".to_string()),
        username: None,
        topic_pattern: "sensor/+/data".to_string(),
        action: AclAction::Publish,
        permission: AclPermission::Allow,
        priority: 10,
    },
    AclRule {
        client_id: None,
        username: Some("admin".to_string()),
        topic_pattern: "#".to_string(),
        action: AclAction::Both,
        permission: AclPermission::Allow,
        priority: 100,
    },
];

let acl = MqttAcl::new(rules);
let manager = MqttManager::new().with_acl(acl);

// 检查权限
if let Some(acl) = manager.acl() {
    let can_publish = acl.check_publish("sensor_001", None, "sensor/room1/data");
    println!("Can publish: {}", can_publish);
}
```

### 使用监控指标

```rust
use flux_mqtt::manager::MqttManager;

let manager = MqttManager::new();

// 获取指标快照
let snapshot = manager.metrics().snapshot();
println!("当前连接数: {}", snapshot.connections_current);
println!("总消息数: {}", snapshot.messages_published);

// 导出 Prometheus 格式
let prometheus = manager.metrics().export_prometheus();
println!("{}", prometheus);
```

### 运行示例

```bash
# 不使用 TLS
cargo run -p flux-mqtt --example mqtt_server

# 使用 TLS
MQTT_TLS_ENABLED=true \
MQTT_CERT_PATH=certs/server.crt \
MQTT_KEY_PATH=certs/server.key \
cargo run -p flux-mqtt --example mqtt_server
```

## 架构设计

### 核心组件

```
flux-mqtt/
├── handler.rs          # MQTT 协议处理器
├── manager.rs          # 会话和订阅管理
├── retained.rs         # Retained 消息存储
├── topic_matcher.rs    # 主题通配符匹配
└── tls.rs             # TLS 配置
```

### 消息流程

```
MQTT Client
    ↓
[MQTT Protocol Handler]
    ↓
[MqttManager]
    ├─> [RetainedStore]      # Retained 消息
    ├─> [TopicMatcher]       # 订阅匹配
    └─> [EventBus]           # 消息转发
```

## QoS 支持

| QoS | 名称 | 支持状态 |
|-----|------|---------|
| 0 | At Most Once | ✅ 支持 |
| 1 | At Least Once | ✅ 支持 |
| 2 | Exactly Once | ⚠️ 降级为 QoS 1 |

## Retained 消息

Retained 消息会保存主题的最后一条消息，新订阅者会立即收到：

```bash
# 发布 retained 消息
mosquitto_pub -h localhost -t "sensor/temp" -m "25.5" -r

# 订阅（立即收到 retained 消息）
mosquitto_sub -h localhost -t "sensor/temp"
# 输出: 25.5
```

## 主题通配符

### 单级通配符 `+`

匹配单个层级：

```bash
# 订阅
mosquitto_sub -h localhost -t "sensor/+/temperature"

# 匹配
sensor/room1/temperature  ✅
sensor/room2/temperature  ✅

# 不匹配
sensor/room1/room2/temperature  ❌
```

### 多级通配符 `#`

匹配多个层级（只能在末尾）：

```bash
# 订阅
mosquitto_sub -h localhost -t "sensor/#"

# 匹配
sensor/temperature        ✅
sensor/room1/temperature  ✅
sensor/room1/room2/temp   ✅

# 不匹配
device/temperature        ❌
```

## TLS/SSL 配置

### 生成自签名证书

```bash
# 生成私钥
openssl genrsa -out server.key 2048

# 生成证书
openssl req -new -x509 -key server.key -out server.crt -days 365
```

### 配置 TLS

```rust
use flux_mqtt::tls::TlsConfig;

let tls_config = TlsConfig::new(
    "certs/server.crt".to_string(),
    "certs/server.key".to_string(),
);

// 可选：启用客户端认证
let tls_config = tls_config.with_client_auth("certs/ca.crt".to_string());
```

### 客户端连接

```bash
# MQTT (无 TLS)
mosquitto_sub -h localhost -p 1883 -t "test/#"

# MQTTS (TLS)
mosquitto_sub -h localhost -p 8883 -t "test/#" \
  --cafile certs/server.crt \
  --insecure
```

## 性能

- **并发连接**: 支持数千并发客户端
- **消息吞吐**: 基于 ntex 高性能异步框架
- **内存效率**: 使用 `Bytes` 零拷贝，`DashMap` 无锁并发

## 配置示例

```toml
[mqtt]
enabled = true
port = 1883
workers = 2

[mqtt.tls]
enabled = true
port = 8883
cert_path = "/etc/flux/certs/server.crt"
key_path = "/etc/flux/certs/server.key"
client_auth = false
```

## 测试

```bash
# 运行单元测试
cargo test -p flux-mqtt

# 运行示例服务器
cargo run -p flux-mqtt --example mqtt_server

# 使用 mosquitto 客户端测试
mosquitto_pub -h localhost -t "test/topic" -m "Hello MQTT"
mosquitto_sub -h localhost -t "test/#"
```

## API 文档

### MqttManager

```rust
impl MqttManager {
    // 发布消息到订阅者
    pub async fn publish_to_subscribers(
        &self,
        topic: &str,
        payload: Bytes,
        retained: bool
    );

    // 订阅主题
    pub async fn subscribe(&self, client_id: &str, topic_filter: &str);

    // 取消订阅
    pub fn unsubscribe(&self, client_id: &str, topic_filter: &str);

    // 访问 retained 消息存储
    pub fn retained_store(&self) -> &RetainedStore;

    // 访问主题匹配器
    pub fn topic_matcher(&self) -> &TopicMatcher;
}
```

### RetainedStore

```rust
impl RetainedStore {
    // 设置 retained 消息
    pub fn set(&self, topic: String, payload: Bytes, qos: u8);

    // 获取 retained 消息
    pub fn get(&self, topic: &str) -> Option<RetainedMessage>;

    // 获取匹配的 retained 消息
    pub fn get_matching(&self, topic_filter: &str) -> Vec<RetainedMessage>;
}
```

### TopicMatcher

```rust
impl TopicMatcher {
    // 订阅主题
    pub fn subscribe(&self, client_id: String, topic_filter: String);

    // 取消订阅
    pub fn unsubscribe(&self, client_id: &str, topic_filter: &str);

    // 查找匹配的客户端
    pub fn find_matching_clients(&self, topic: &str) -> Vec<String>;

    // 主题匹配
    pub fn matches(filter: &str, topic: &str) -> bool;
}
```

## 故障排查

### 连接失败

```bash
# 检查端口是否监听
netstat -an | grep 1883
netstat -an | grep 8883

# 检查防火墙
sudo ufw allow 1883
sudo ufw allow 8883
```

### TLS 错误

```bash
# 验证证书
openssl x509 -in server.crt -text -noout

# 测试 TLS 连接
openssl s_client -connect localhost:8883
```

### 日志调试

```bash
# 启用详细日志
RUST_LOG=debug cargo run -p flux-mqtt --example mqtt_server
```

## 依赖

- `ntex` - 异步网络框架
- `ntex-mqtt` - MQTT 协议实现
- `tokio` - 异步运行时
- `rustls` - TLS 库
- `dashmap` - 并发哈希表
- `flux-core` - 核心功能（EventBus, Authenticator）

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！

## 路线图

### 短期（1-2 周）
- [ ] Will 消息支持
- [ ] 持久化会话
- [ ] 访问控制 ACL

### 中期（1-2 月）
- [ ] QoS 2 完整支持
- [ ] WebSocket 支持
- [ ] 监控指标

### 长期（3-6 月）
- [ ] 集群支持
- [ ] 消息桥接
- [ ] 规则引擎集成
