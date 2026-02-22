# MQTT 协议未完善功能清单

> **分析日期**: 2026-02-22  
> **当前状态**: 基础功能已实现，高级特性待完善  
> **完成度**: 40%

---

## 📊 总体评估

### 已实现功能 ✅

| 功能 | 状态 | 说明 |
|------|------|------|
| MQTT v3.1.1 支持 | ✅ 完成 | 基础协议实现 |
| MQTT v5.0 支持 | ✅ 完成 | 基础协议实现 |
| 客户端连接 | ✅ 完成 | 支持 v3/v5 |
| 认证集成 | ✅ 完成 | 集成 Authenticator |
| 发布/订阅 | ✅ 部分 | 仅 QoS 1 |
| 会话管理 | ✅ 部分 | 内存存储 |
| EventBus 集成 | ✅ 完成 | 双向消息转发 |
| TLS 配置 | ✅ 完成 | 配置代码已实现 |

### 未实现/不完善功能 ❌

| 功能 | 状态 | 优先级 |
|------|------|--------|
| QoS 0 支持 | ❌ 未实现 | 🔥 高 |
| QoS 2 支持 | ❌ 未实现 | 🔥 高 |
| Retained 消息 | ❌ 未实现 | 🔥 高 |
| Will 消息 | ❌ 未实现 | 🟡 中 |
| 持久化会话 | ❌ 未实现 | 🔥 高 |
| 离线消息队列 | ❌ 未实现 | 🔥 高 |
| 主题通配符 | ❌ 未实现 | 🔥 高 |
| 共享订阅 | ❌ 未实现 | 🟡 中 |
| TLS 实际启用 | ❌ 未实现 | 🔥 高 |
| WebSocket 支持 | ❌ 未实现 | 🟡 中 |
| 消息桥接 | ⚠️ 部分 | 🟡 中 |
| 监控指标 | ❌ 未实现 | 🟡 中 |
| 访问控制 ACL | ❌ 未实现 | 🔥 高 |
| 消息持久化 | ❌ 未实现 | 🔥 高 |
| 集群支持 | ❌ 未实现 | 🟢 低 |

---

## 🔥 高优先级功能（生产必需）

### 1. QoS 完整支持 ⚠️

**当前状态**: 仅支持 QoS 1（At Least Once）

**代码位置**:
- `src/handler.rs:101` - V3 订阅固定为 QoS 1
- `src/handler.rs:184` - V5 订阅固定为 QoS 1

**待实现**:

#### 1.1 QoS 0 (At Most Once)
```rust
// handler.rs
v3::Control::Protocol(v3::CtlFrame::Subscribe(mut sub)) => {
    for mut s in &mut sub {
        // TODO: 根据客户端请求的 QoS 设置
        let requested_qos = s.qos();
        match requested_qos {
            v3::QoS::AtMostOnce => s.subscribe(v3::QoS::AtMostOnce),
            v3::QoS::AtLeastOnce => s.subscribe(v3::QoS::AtLeastOnce),
            v3::QoS::ExactlyOnce => s.subscribe(v3::QoS::AtLeastOnce), // 降级
        }
    }
    Ok(sub.ack())
}
```

#### 1.2 QoS 2 (Exactly Once)
```rust
// 需要实现消息去重和确认流程
// 1. PUBLISH -> PUBREC -> PUBREL -> PUBCOMP
// 2. 消息 ID 追踪
// 3. 重复消息检测
```

**预计工期**: 3-5 天  
**优先级**: 🔥 高

---

### 2. Retained 消息 ❌

**当前状态**: 未实现

**功能说明**: 
- 保存主题的最后一条消息
- 新订阅者立即收到保留消息
- 用于设备状态同步

**实现方案**:

```rust
// manager.rs
pub struct MqttManager {
    sessions: Rc<RefCell<HashMap<String, SessionState>>>,
    // 新增：保留消息存储
    retained_messages: Rc<RefCell<HashMap<String, RetainedMessage>>>,
}

pub struct RetainedMessage {
    pub topic: String,
    pub payload: Bytes,
    pub qos: QoS,
    pub timestamp: SystemTime,
}

impl MqttManager {
    // 保存保留消息
    pub fn set_retained(&self, topic: String, payload: Bytes, qos: QoS) {
        self.retained_messages.borrow_mut().insert(
            topic,
            RetainedMessage {
                topic: topic.clone(),
                payload,
                qos,
                timestamp: SystemTime::now(),
            },
        );
    }
    
    // 获取保留消息
    pub fn get_retained(&self, topic: &str) -> Option<RetainedMessage> {
        self.retained_messages.borrow().get(topic).cloned()
    }
    
    // 订阅时发送保留消息
    pub async fn send_retained_on_subscribe(&self, topic: &str, sink: &MqttSink) {
        if let Some(msg) = self.get_retained(topic) {
            sink.publish(&msg.topic, msg.payload).await;
        }
    }
}
```

**预计工期**: 2-3 天  
**优先级**: 🔥 高

---

### 3. 主题通配符支持 ❌

**当前状态**: 未实现

**MQTT 通配符**:
- `+` - 单级通配符 (e.g., `sensor/+/temperature`)
- `#` - 多级通配符 (e.g., `sensor/#`)

**实现方案**:

```rust
// 新增 topic_matcher.rs
pub struct TopicMatcher {
    subscriptions: HashMap<String, Vec<String>>, // topic_pattern -> client_ids
}

impl TopicMatcher {
    pub fn matches(pattern: &str, topic: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let topic_parts: Vec<&str> = topic.split('/').collect();
        
        self.matches_parts(&pattern_parts, &topic_parts)
    }
    
    fn matches_parts(&self, pattern: &[&str], topic: &[&str]) -> bool {
        match (pattern.first(), topic.first()) {
            (None, None) => true,
            (Some(&"#"), _) => true,
            (Some(&"+"), Some(_)) => {
                self.matches_parts(&pattern[1..], &topic[1..])
            }
            (Some(p), Some(t)) if p == t => {
                self.matches_parts(&pattern[1..], &topic[1..])
            }
            _ => false,
        }
    }
    
    pub fn find_matching_clients(&self, topic: &str) -> Vec<String> {
        let mut clients = Vec::new();
        for (pattern, client_ids) in &self.subscriptions {
            if Self::matches(pattern, topic) {
                clients.extend(client_ids.clone());
            }
        }
        clients
    }
}
```

**预计工期**: 3-4 天  
**优先级**: 🔥 高

---

### 4. 持久化会话 ❌

**当前状态**: 仅内存存储，重启丢失

**功能说明**:
- Clean Session = false 时保存会话
- 保存订阅信息
- 保存未确认消息
- 保存离线消息

**实现方案**:

```rust
// 使用 SeaORM 持久化
pub struct SessionStore {
    db: Arc<DatabaseConnection>,
}

// 会话表
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "mqtt_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub client_id: String,
    pub clean_session: bool,
    pub subscriptions: Json, // Vec<Subscription>
    pub created_at: DateTime,
    pub last_seen: DateTime,
}

// 离线消息表
#[sea_orm(table_name = "mqtt_offline_messages")]
pub struct OfflineMessage {
    pub id: i64,
    pub client_id: String,
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: i16,
    pub retained: bool,
    pub created_at: DateTime,
}
```

**数据库迁移**:
```sql
CREATE TABLE mqtt_sessions (
    client_id VARCHAR(255) PRIMARY KEY,
    clean_session BOOLEAN NOT NULL,
    subscriptions JSONB,
    created_at TIMESTAMP NOT NULL,
    last_seen TIMESTAMP NOT NULL
);

CREATE TABLE mqtt_offline_messages (
    id BIGSERIAL PRIMARY KEY,
    client_id VARCHAR(255) NOT NULL,
    topic VARCHAR(255) NOT NULL,
    payload BYTEA NOT NULL,
    qos SMALLINT NOT NULL,
    retained BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL,
    INDEX idx_client_id (client_id),
    INDEX idx_created_at (created_at)
);

CREATE TABLE mqtt_retained_messages (
    topic VARCHAR(255) PRIMARY KEY,
    payload BYTEA NOT NULL,
    qos SMALLINT NOT NULL,
    created_at TIMESTAMP NOT NULL
);
```

**预计工期**: 5-7 天  
**优先级**: 🔥 高

---

### 5. TLS/SSL 实际启用 ⚠️

**当前状态**: TLS 配置代码已实现，但未在 broker 中启用

**代码位置**: `src/tls.rs` - 配置已完成

**待完成**:

```rust
// lib.rs - 添加 TLS 支持
pub fn start_broker_with_tls(
    event_bus: Arc<EventBus>,
    authenticator: Arc<dyn Authenticator>,
    tls_config: Option<TlsConfig>,
) {
    thread::spawn(move || {
        let _ = run_mqtt_server_tls(event_bus, authenticator, tls_config);
    });
}

#[ntex::main]
async fn run_mqtt_server_tls(
    event_bus: Arc<EventBus>,
    authenticator: Arc<dyn Authenticator>,
    tls_config: Option<TlsConfig>,
) -> std::io::Result<()> {
    let mut server = ntex::server::build();
    
    // 标准 MQTT (1883)
    server = server.bind("mqtt", "0.0.0.0:1883", move |_| {
        // ... 现有代码
    })?;
    
    // MQTTS (8883)
    if let Some(tls_cfg) = tls_config {
        let rustls_config = crate::tls::load_tls_config(&tls_cfg)
            .expect("Failed to load TLS config");
        
        server = server.bind("mqtts", "0.0.0.0:8883", move |_| {
            // ... 使用 TLS 的 MQTT 服务器
        })?
        .rustls(rustls_config);
    }
    
    server.workers(2).run().await
}
```

**配置文件支持**:
```toml
# config.toml
[mqtt]
enabled = true
port = 1883

[mqtt.tls]
enabled = true
port = 8883
cert_path = "/etc/flux/certs/mqtt.crt"
key_path = "/etc/flux/certs/mqtt.key"
client_auth = false
```

**预计工期**: 2-3 天  
**优先级**: 🔥 高

---

### 6. 访问控制 ACL ❌

**当前状态**: 未实现主题级别的权限控制

**功能说明**:
- 控制客户端可以订阅/发布的主题
- 基于用户/角色的权限
- 主题模式匹配

**实现方案**:

```rust
// acl.rs
pub struct MqttAcl {
    rules: Vec<AclRule>,
}

pub struct AclRule {
    pub client_id: Option<String>,
    pub username: Option<String>,
    pub topic_pattern: String,
    pub action: AclAction,
    pub permission: AclPermission,
}

pub enum AclAction {
    Publish,
    Subscribe,
    Both,
}

pub enum AclPermission {
    Allow,
    Deny,
}

impl MqttAcl {
    pub fn check_publish(&self, client_id: &str, topic: &str) -> bool {
        self.check_permission(client_id, topic, AclAction::Publish)
    }
    
    pub fn check_subscribe(&self, client_id: &str, topic: &str) -> bool {
        self.check_permission(client_id, topic, AclAction::Subscribe)
    }
    
    fn check_permission(&self, client_id: &str, topic: &str, action: AclAction) -> bool {
        // 匹配规则并检查权限
        for rule in &self.rules {
            if self.matches_rule(rule, client_id, topic, &action) {
                return matches!(rule.permission, AclPermission::Allow);
            }
        }
        false // 默认拒绝
    }
}
```

**配置示例**:
```toml
[[mqtt.acl]]
client_id = "sensor_*"
topic_pattern = "sensor/+/data"
action = "publish"
permission = "allow"

[[mqtt.acl]]
username = "admin"
topic_pattern = "#"
action = "both"
permission = "allow"
```

**预计工期**: 4-5 天  
**优先级**: 🔥 高

---

## 🟡 中优先级功能

### 7. Will 消息（遗嘱消息）❌

**功能说明**: 客户端异常断开时自动发布的消息

**实现方案**:
```rust
pub struct WillMessage {
    pub topic: String,
    pub payload: Bytes,
    pub qos: QoS,
    pub retained: bool,
}

// 在 SessionState 中保存
pub struct SessionState {
    pub client_id: String,
    pub sink: MqttSink,
    pub will: Option<WillMessage>, // 新增
}

// 断开连接时发送
pub async fn on_disconnect(&self, client_id: &str, abnormal: bool) {
    if abnormal {
        if let Some(session) = self.sessions.borrow().get(client_id) {
            if let Some(will) = &session.will {
                self.broadcast(&will.topic, will.payload.clone()).await;
            }
        }
    }
    self.remove(client_id);
}
```

**预计工期**: 2-3 天  
**优先级**: 🟡 中

---

### 8. WebSocket 支持 ❌

**功能说明**: MQTT over WebSocket (用于浏览器客户端)

**实现方案**:
```rust
// 使用 ntex-ws
use ntex_ws as ws;

// 添加 WebSocket 端点
server.bind("mqtt-ws", "0.0.0.0:8083", |_| {
    ws::WsServer::new(|req| async move {
        // WebSocket 握手
        let (res, framed) = req.into_response()?;
        
        // MQTT over WebSocket 协议处理
        // ...
        
        Ok(res)
    })
})?;
```

**预计工期**: 4-5 天  
**优先级**: 🟡 中

---

### 9. 共享订阅 ❌

**功能说明**: 多个客户端共享同一订阅，负载均衡

**MQTT 5.0 语法**: `$share/{group}/{topic}`

**实现方案**:
```rust
pub struct SharedSubscription {
    pub group: String,
    pub topic: String,
    pub clients: Vec<String>,
    pub next_index: usize, // 轮询索引
}

impl MqttManager {
    pub async fn publish_to_shared(&self, topic: &str, payload: Bytes) {
        // 解析共享订阅
        if topic.starts_with("$share/") {
            let parts: Vec<&str> = topic.splitn(3, '/').collect();
            if parts.len() == 3 {
                let group = parts[1];
                let actual_topic = parts[2];
                
                // 轮询发送给组内的一个客户端
                self.send_to_one_in_group(group, actual_topic, payload).await;
                return;
            }
        }
        
        // 普通发布
        self.broadcast(topic, payload).await;
    }
}
```

**预计工期**: 3-4 天  
**优先级**: 🟡 中

---

### 10. 监控指标 ❌

**功能说明**: 暴露 MQTT broker 运行指标

**指标项**:
- 连接数（当前/峰值/总计）
- 消息数（发布/接收/丢弃）
- 订阅数
- 字节数（发送/接收）
- QoS 分布
- 错误数

**实现方案**:
```rust
use prometheus::{Counter, Gauge, Histogram, Registry};

pub struct MqttMetrics {
    pub connections_current: Gauge,
    pub connections_total: Counter,
    pub messages_published: Counter,
    pub messages_received: Counter,
    pub messages_dropped: Counter,
    pub bytes_sent: Counter,
    pub bytes_received: Counter,
    pub publish_duration: Histogram,
}

impl MqttMetrics {
    pub fn new(registry: &Registry) -> Self {
        // 注册指标...
    }
    
    pub fn record_connection(&self) {
        self.connections_current.inc();
        self.connections_total.inc();
    }
    
    pub fn record_publish(&self, bytes: usize, duration: Duration) {
        self.messages_published.inc();
        self.bytes_sent.inc_by(bytes as f64);
        self.publish_duration.observe(duration.as_secs_f64());
    }
}
```

**暴露端点**:
```rust
// HTTP /metrics 端点
async fn metrics_handler() -> String {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder.encode_to_string(&metric_families).unwrap()
}
```

**预计工期**: 3-4 天  
**优先级**: 🟡 中

---

## 🟢 低优先级功能

### 11. 集群支持 ❌

**功能说明**: 多个 broker 节点组成集群

**技术方案**:
- 使用 Redis 作为消息总线
- 节点间消息转发
- 会话共享
- 负载均衡

**预计工期**: 2-3 周  
**优先级**: 🟢 低

---

## 📋 实施优先级建议

### 第一阶段（1-2 周）🔥
1. **QoS 0/2 支持** (3-5 天)
2. **Retained 消息** (2-3 天)
3. **主题通配符** (3-4 天)
4. **TLS 启用** (2-3 天)

### 第二阶段（2-3 周）🔥
5. **持久化会话** (5-7 天)
6. **访问控制 ACL** (4-5 天)
7. **监控指标** (3-4 天)

### 第三阶段（1-2 周）🟡
8. **Will 消息** (2-3 天)
9. **WebSocket 支持** (4-5 天)
10. **共享订阅** (3-4 天)

### 第四阶段（可选）🟢
11. **集群支持** (2-3 周)

---

## 📊 完成度评估

| 类别 | 完成度 | 说明 |
|------|--------|------|
| **基础协议** | 70% | v3/v5 基础功能完成 |
| **QoS 支持** | 33% | 仅 QoS 1 |
| **高级特性** | 10% | 大部分未实现 |
| **持久化** | 0% | 完全未实现 |
| **安全性** | 50% | 认证完成，ACL 未实现 |
| **监控** | 0% | 未实现 |
| **总体** | **40%** | 基础可用，生产不足 |

---

## 🎯 总结

**已完成**:
- ✅ MQTT v3.1.1 / v5.0 基础协议
- ✅ 客户端连接和认证
- ✅ 基础发布/订阅 (QoS 1)
- ✅ EventBus 集成
- ✅ TLS 配置代码

**关键缺失**:
- ❌ 完整的 QoS 支持
- ❌ Retained 消息
- ❌ 主题通配符
- ❌ 持久化会话
- ❌ 访问控制 ACL
- ❌ 监控指标

**建议**:
优先实施第一阶段和第二阶段的功能，这些是生产环境必需的。第三阶段可根据实际需求选择性实施。

---

**维护者**: FLUX IOT Team  
**分析日期**: 2026-02-22  
**下一步**: 开始实施第一阶段功能
