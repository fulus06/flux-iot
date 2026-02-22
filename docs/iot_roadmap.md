# FLUX IOT 通用物联网平台发展路线图

> **版本**: v1.0  
> **日期**: 2026-02-22  
> **发展方向**: 通用物联网平台（保留流媒体优势）  
> **目标**: 打造全栈物联网解决方案

---

## 🎯 总体目标

构建一个**全功能的通用物联网平台**，同时保持现有的流媒体优势，支持：

- ✅ **视频物联网**：摄像头、视频监控、视频分析
- ✅ **通用物联网**：传感器、执行器、智能设备
- ✅ **工业物联网**：工业设备、生产监控、数据采集
- ✅ **边缘计算**：边缘网关、本地处理、边云协同

---

## 📊 当前状态

### 已有优势
- ✅ 流媒体功能完善（RTMP/RTSP/SRT/HLS/HTTP-FLV）
- ✅ 安全功能完善（JWT/RBAC/限流/会话管理）
- ✅ 插件系统灵活（Wasm/Rhai）
- ✅ 配置管理完善（热更新/版本管理）
- ✅ 监控告警完善（Prometheus/Grafana）

### 核心缺失
- ❌ 设备管理系统（0%）
- ❌ 设备控制功能（0%）
- ❌ 物联网协议支持不足（40%）
- ❌ 数据可视化（0%）
- ❌ 边缘计算（0%）

---

## 🗓️ 实施计划

### 阶段 1：设备管理系统（2 周）🔥 **优先**

**目标**：建立完整的设备生命周期管理

#### 1.1 创建 flux-device 包（3 天）

**功能**：
- [x] 设备数据模型定义
- [x] 设备注册/注销
- [x] 设备认证（证书/密钥）
- [x] 设备分组管理
- [x] 设备标签系统

**代码结构**：
```
flux-device/
  ├─ src/
  │   ├─ model.rs          # 设备数据模型
  │   ├─ registry.rs       # 设备注册表
  │   ├─ auth.rs           # 设备认证
  │   ├─ group.rs          # 设备分组
  │   └─ manager.rs        # 设备管理器
  └─ Cargo.toml
```

**数据模型**：
```rust
pub struct Device {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub protocol: Protocol,
    pub status: DeviceStatus,
    pub metadata: HashMap<String, String>,
    pub tags: Vec<String>,
    pub group_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum DeviceType {
    Camera,        // 摄像头
    Sensor,        // 传感器
    Actuator,      // 执行器
    Gateway,       // 网关
    Industrial,    // 工业设备
    Custom(String),
}

pub enum DeviceStatus {
    Online,
    Offline,
    Fault,
    Maintenance,
}
```

---

#### 1.2 设备状态监控（2 天）

**功能**：
- [x] 心跳检测机制
- [x] 在线/离线状态追踪
- [x] 设备健康检查
- [x] 故障告警
- [x] 性能指标收集

**实现**：
```rust
pub struct DeviceMonitor {
    devices: Arc<RwLock<HashMap<String, DeviceState>>>,
    heartbeat_interval: Duration,
    timeout: Duration,
}

impl DeviceMonitor {
    pub async fn check_heartbeat(&self, device_id: &str);
    pub async fn update_status(&self, device_id: &str, status: DeviceStatus);
    pub async fn get_metrics(&self, device_id: &str) -> DeviceMetrics;
}
```

---

#### 1.3 设备 API（2 天）

**RESTful API**：
```
POST   /api/v1/devices              # 注册设备
GET    /api/v1/devices              # 列出设备
GET    /api/v1/devices/:id          # 设备详情
PUT    /api/v1/devices/:id          # 更新设备
DELETE /api/v1/devices/:id          # 删除设备
POST   /api/v1/devices/:id/activate # 激活设备
GET    /api/v1/devices/:id/status   # 设备状态
GET    /api/v1/devices/:id/metrics  # 设备指标

# 设备分组
POST   /api/v1/device-groups        # 创建分组
GET    /api/v1/device-groups        # 列出分组
GET    /api/v1/device-groups/:id/devices  # 分组设备
```

---

#### 1.4 数据库设计（1 天）

**表结构**：
```sql
-- 设备表
CREATE TABLE devices (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    device_type VARCHAR(50) NOT NULL,
    protocol VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL,
    metadata JSONB,
    tags TEXT[],
    group_id VARCHAR(64),
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    FOREIGN KEY (group_id) REFERENCES device_groups(id)
);

-- 设备分组表
CREATE TABLE device_groups (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    parent_id VARCHAR(64),
    created_at TIMESTAMP NOT NULL,
    FOREIGN KEY (parent_id) REFERENCES device_groups(id)
);

-- 设备状态历史表
CREATE TABLE device_status_history (
    id BIGSERIAL PRIMARY KEY,
    device_id VARCHAR(64) NOT NULL,
    status VARCHAR(20) NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    metadata JSONB,
    FOREIGN KEY (device_id) REFERENCES devices(id)
);

-- 设备指标表
CREATE TABLE device_metrics (
    id BIGSERIAL PRIMARY KEY,
    device_id VARCHAR(64) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    FOREIGN KEY (device_id) REFERENCES devices(id)
);
```

---

### 阶段 2：MQTT 协议完善（1 周）🔥

**目标**：实现完整的 MQTT 3.1.1 和 MQTT 5.0 支持

#### 2.1 MQTT QoS 1/2 实现（3 天）

**功能**：
- [x] QoS 1（至少一次）
- [x] QoS 2（恰好一次）
- [x] 消息确认机制
- [x] 消息重传
- [x] 消息去重

**实现要点**：
```rust
// QoS 1: 发布确认
pub async fn publish_qos1(&self, topic: &str, payload: Bytes) -> Result<()> {
    let packet_id = self.next_packet_id();
    self.send_publish(topic, payload, QoS::AtLeastOnce, packet_id).await?;
    self.wait_for_puback(packet_id).await?;
    Ok(())
}

// QoS 2: 四次握手
pub async fn publish_qos2(&self, topic: &str, payload: Bytes) -> Result<()> {
    let packet_id = self.next_packet_id();
    self.send_publish(topic, payload, QoS::ExactlyOnce, packet_id).await?;
    self.wait_for_pubrec(packet_id).await?;
    self.send_pubrel(packet_id).await?;
    self.wait_for_pubcomp(packet_id).await?;
    Ok(())
}
```

---

#### 2.2 MQTT 高级特性（2 天）

**功能**：
- [x] 遗嘱消息（Last Will）
- [x] 保留消息（Retained Messages）
- [x] 会话保持（Clean Session）
- [x] 主题通配符（+/#）
- [x] 共享订阅

---

#### 2.3 MQTT 5.0 支持（2 天）

**新特性**：
- [x] 用户属性
- [x] 请求/响应模式
- [x] 主题别名
- [x] 消息过期
- [x] 订阅选项

---

### 阶段 3：设备控制功能（1-2 周）🔥

**目标**：实现完整的设备远程控制

#### 3.1 创建 flux-control 包（3 天）

**功能**：
- [x] 指令定义和封装
- [x] 指令队列管理
- [x] 指令下发机制
- [x] 指令执行追踪
- [x] 指令超时处理

**代码结构**：
```rust
pub struct DeviceCommand {
    pub id: String,
    pub device_id: String,
    pub command_type: CommandType,
    pub params: HashMap<String, Value>,
    pub timeout: Duration,
    pub status: CommandStatus,
    pub created_at: DateTime<Utc>,
}

pub enum CommandType {
    // 通用指令
    Reboot,
    Reset,
    Update,
    
    // 摄像头指令
    StartStream,
    StopStream,
    TakeSnapshot,
    PTZControl { pan: i32, tilt: i32, zoom: i32 },
    
    // 传感器指令
    ReadValue,
    SetSamplingRate { rate: u32 },
    
    // 执行器指令
    SetState { state: bool },
    SetValue { value: f64 },
    
    Custom { name: String, params: Value },
}

pub enum CommandStatus {
    Pending,
    Sent,
    Executing,
    Success,
    Failed { error: String },
    Timeout,
}
```

---

#### 3.2 场景联动（3 天）

**功能**：
- [x] 场景定义（IF-THEN-ELSE）
- [x] 多设备联动
- [x] 定时任务
- [x] 条件触发

**场景示例**：
```rust
pub struct Scene {
    pub id: String,
    pub name: String,
    pub triggers: Vec<Trigger>,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub enabled: bool,
}

// 示例：温度过高时开启风扇
let scene = Scene {
    id: "temp_control".to_string(),
    name: "温度控制".to_string(),
    triggers: vec![
        Trigger::DeviceData {
            device_id: "temp_sensor_01".to_string(),
            metric: "temperature".to_string(),
            operator: Operator::GreaterThan,
            value: 30.0,
        }
    ],
    conditions: vec![],
    actions: vec![
        Action::DeviceCommand {
            device_id: "fan_01".to_string(),
            command: CommandType::SetState { state: true },
        }
    ],
    enabled: true,
};
```

---

#### 3.3 控制 API（2 天）

**RESTful API**：
```
POST   /api/v1/devices/:id/commands     # 发送指令
GET    /api/v1/devices/:id/commands     # 指令历史
GET    /api/v1/commands/:cmd_id         # 指令状态
DELETE /api/v1/commands/:cmd_id         # 取消指令
POST   /api/v1/devices/batch/commands   # 批量控制

# 场景管理
POST   /api/v1/scenes                   # 创建场景
GET    /api/v1/scenes                   # 列出场景
PUT    /api/v1/scenes/:id               # 更新场景
DELETE /api/v1/scenes/:id               # 删除场景
POST   /api/v1/scenes/:id/execute       # 手动执行场景
```

---

### 阶段 4：数据存储优化（1 周）

**目标**：支持海量时序数据存储

#### 4.1 时序数据库集成（3 天）

**选型**：
- **InfluxDB**: 专业时序数据库
- **TimescaleDB**: PostgreSQL 扩展

**实现**：
```rust
// 创建 flux-timeseries 包
pub trait TimeSeriesStore {
    async fn write_point(&self, point: DataPoint) -> Result<()>;
    async fn write_batch(&self, points: Vec<DataPoint>) -> Result<()>;
    async fn query(&self, query: TimeSeriesQuery) -> Result<Vec<DataPoint>>;
}

pub struct DataPoint {
    pub measurement: String,
    pub tags: HashMap<String, String>,
    pub fields: HashMap<String, Value>,
    pub timestamp: DateTime<Utc>,
}
```

---

#### 4.2 数据归档策略（2 天）

**功能**：
- [x] 数据保留策略（Retention Policy）
- [x] 数据降采样（Downsampling）
- [x] 冷热数据分离
- [x] 自动归档任务

**配置示例**：
```toml
[timeseries]
# 原始数据保留 7 天
raw_retention = "7d"

# 1分钟聚合数据保留 30 天
minute_retention = "30d"

# 1小时聚合数据保留 1 年
hour_retention = "365d"

# 1天聚合数据永久保留
day_retention = "inf"
```

---

#### 4.3 数据清理（2 天）

**功能**：
- [x] 过期数据自动清理
- [x] 数据压缩
- [x] 存储空间监控
- [x] 清理任务调度

---

### 阶段 5：协议扩展（3-4 周）

**目标**：支持主流物联网协议

#### 5.1 CoAP 协议（1 周）

**功能**：
- [x] CoAP 服务器
- [x] 资源发现
- [x] 观察者模式
- [x] 块传输

**创建包**：`flux-coap`

---

#### 5.2 Modbus 协议（1 周）

**功能**：
- [x] Modbus TCP
- [x] Modbus RTU
- [x] 主站/从站模式
- [x] 寄存器读写

**创建包**：`flux-modbus`

---

#### 5.3 OPC UA 协议（1-2 周）

**功能**：
- [x] OPC UA 客户端
- [x] 节点浏览
- [x] 数据订阅
- [x] 历史数据访问

**创建包**：`flux-opcua`

---

### 阶段 6：数据可视化（3 周）

**目标**：提供完整的数据可视化能力

#### 6.1 实时数据大屏（1 周）

**技术栈**：
- 前端：React + TypeScript + ECharts
- 实时通信：WebSocket
- 地图：Leaflet/Mapbox

**功能**：
- [x] 设备状态地图
- [x] 实时数据图表
- [x] 告警展示
- [x] 自定义布局

---

#### 6.2 历史数据查询（1 周）

**功能**：
- [x] 时间范围查询
- [x] 多维度筛选
- [x] 数据对比
- [x] 趋势分析
- [x] 数据导出

---

#### 6.3 报表系统（1 周）

**功能**：
- [x] 报表模板
- [x] 定时报表
- [x] 报表导出（PDF/Excel）
- [x] 报表分发

---

### 阶段 7：边缘计算（4-6 周）

**目标**：支持边缘节点部署

#### 7.1 边缘网关（2 周）

**功能**：
- [x] 边缘节点管理
- [x] 本地数据处理
- [x] 离线缓存
- [x] 边云同步

---

#### 7.2 边缘规则引擎（2 周）

**功能**：
- [x] 本地规则执行
- [x] 低延迟响应
- [x] 离线自治

---

#### 7.3 边缘 AI（2 周）

**功能**：
- [x] 模型部署
- [x] 本地推理
- [x] 视频分析

---

## 📅 时间线

```
Month 1 (Week 1-4):
├─ Week 1-2: 设备管理系统 ✅
├─ Week 3: MQTT 协议完善 ✅
└─ Week 4: 设备控制功能（开始）

Month 2 (Week 5-8):
├─ Week 5: 设备控制功能（完成）
├─ Week 6: 数据存储优化
├─ Week 7-8: CoAP 协议

Month 3 (Week 9-12):
├─ Week 9-10: Modbus 协议
├─ Week 11-12: OPC UA 协议（开始）

Month 4 (Week 13-16):
├─ Week 13: OPC UA 协议（完成）
├─ Week 14-16: 数据可视化

Month 5-6 (Week 17-24):
└─ Week 17-24: 边缘计算
```

---

## 🎯 里程碑

### M1: 基础物联网平台（Month 1）
- ✅ 设备管理系统
- ✅ MQTT 完善
- ✅ 设备控制
- **可用性**: 支持基础设备接入和控制

### M2: 协议扩展（Month 2-3）
- ✅ CoAP 支持
- ✅ Modbus 支持
- ✅ 数据存储优化
- **可用性**: 支持工业设备接入

### M3: 可视化平台（Month 4）
- ✅ OPC UA 支持
- ✅ 数据可视化
- **可用性**: 完整的监控平台

### M4: 边缘计算（Month 5-6）
- ✅ 边缘网关
- ✅ 边缘规则
- ✅ 边缘 AI
- **可用性**: 边云协同平台

---

## 🏗️ 最终架构

```
┌─────────────────────────────────────────────────────────────┐
│                    FLUX IOT 通用物联网平台                    │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Web UI      │  │  Mobile App  │  │  OpenAPI     │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         └──────────────────┴──────────────────┘              │
│                            ↓                                 │
│         ┌──────────────────────────────────┐                │
│         │        API Gateway               │                │
│         │  (认证/授权/限流/路由)            │                │
│         └──────────────┬───────────────────┘                │
│                        ↓                                     │
│  ┌─────────────────────────────────────────────────┐        │
│  │              核心服务层                          │        │
│  ├─────────────────────────────────────────────────┤        │
│  │ 设备管理 │ 数据采集 │ 规则引擎 │ 设备控制 │      │        │
│  │ 数据存储 │ 告警通知 │ 场景联动 │ 任务调度 │      │        │
│  └─────────────────────────────────────────────────┘        │
│                        ↓                                     │
│  ┌─────────────────────────────────────────────────┐        │
│  │              协议适配层                          │        │
│  ├─────────────────────────────────────────────────┤        │
│  │ MQTT │ CoAP │ Modbus │ OPC UA │ HTTP │ WebSocket│        │
│  │ RTMP │ RTSP │ SRT    │ HLS    │ GB28181│ ONVIF  │        │
│  └─────────────────────────────────────────────────┘        │
│                        ↓                                     │
│  ┌─────────────────────────────────────────────────┐        │
│  │              设备接入层                          │        │
│  ├─────────────────────────────────────────────────┤        │
│  │ 摄像头 │ 传感器 │ 执行器 │ 网关 │ 工业设备 │    │        │
│  └─────────────────────────────────────────────────┘        │
│                                                               │
│  ┌─────────────────────────────────────────────────┐        │
│  │              边缘计算层                          │        │
│  ├─────────────────────────────────────────────────┤        │
│  │ 边缘网关 │ 本地处理 │ 离线缓存 │ 边缘AI  │      │        │
│  └─────────────────────────────────────────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

---

## 📦 包结构规划

```
flux-iot/
  ├─ crates/
  │   ├─ flux-device/          # 设备管理 ✨ 新增
  │   ├─ flux-control/         # 设备控制 ✨ 新增
  │   ├─ flux-timeseries/      # 时序数据 ✨ 新增
  │   ├─ flux-coap/            # CoAP 协议 ✨ 新增
  │   ├─ flux-modbus/          # Modbus 协议 ✨ 新增
  │   ├─ flux-opcua/           # OPC UA 协议 ✨ 新增
  │   ├─ flux-edge/            # 边缘计算 ✨ 新增
  │   ├─ flux-visualization/   # 数据可视化 ✨ 新增
  │   │
  │   ├─ flux-middleware/      # 统一中间件 ✅ 已完成
  │   ├─ flux-stream/          # 流管理 ✅ 已完成
  │   ├─ flux-mqtt/            # MQTT 服务 ⚠️ 需完善
  │   ├─ flux-rtmpd/           # RTMP 服务 ✅
  │   ├─ flux-rtspd/           # RTSP 服务 ✅
  │   ├─ flux-srt/             # SRT 协议 ✅
  │   ├─ flux-gb28181d/        # GB28181 ⚠️
  │   ├─ flux-onvif/           # ONVIF ⚠️
  │   │
  │   ├─ flux-core/            # 核心模块 ✅
  │   ├─ flux-config/          # 配置管理 ✅
  │   ├─ flux-storage/         # 存储抽象 ✅
  │   ├─ flux-metrics/         # 监控告警 ✅
  │   ├─ flux-logging/         # 日志增强 ✅
  │   └─ flux-shutdown/        # 优雅关闭 ✅
  │
  └─ web-ui/                   # Web 管理界面 ✨ 新增
      ├─ dashboard/            # 数据大屏
      ├─ device-management/    # 设备管理
      └─ data-visualization/   # 数据可视化
```

---

## 🎓 技术选型

### 后端
- **语言**: Rust 1.75+
- **异步运行时**: Tokio
- **Web 框架**: Axum
- **ORM**: SeaORM
- **时序数据库**: InfluxDB / TimescaleDB
- **消息队列**: MQTT / Redis Streams

### 前端
- **框架**: React 18 + TypeScript
- **UI 库**: Ant Design / shadcn/ui
- **图表**: ECharts / Recharts
- **地图**: Leaflet / Mapbox
- **状态管理**: Redux Toolkit / Zustand

### 协议库
- **MQTT**: rumqttc
- **CoAP**: coap-lite
- **Modbus**: tokio-modbus
- **OPC UA**: opcua

---

## 🚀 立即开始

### 第一步：设备管理系统

**创建 flux-device 包**：
```bash
cargo new --lib crates/flux-device
```

**预计完成时间**: 2 周  
**负责人**: 待分配  
**优先级**: 🔥 最高

---

## 📝 备注

1. **保持现有优势**：流媒体功能继续维护和优化
2. **渐进式开发**：按阶段实施，每个阶段都可独立使用
3. **文档先行**：每个模块都要有完整文档
4. **测试驱动**：每个功能都要有单元测试和集成测试
5. **性能优先**：关注性能指标，定期进行压测

---

**维护者**: FLUX IOT Team  
**最后更新**: 2026-02-22  
**下次审查**: 2026-03-22
