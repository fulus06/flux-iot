# 设备管理系统实现方案

> **版本**: v1.0  
> **日期**: 2026-02-22  
> **预计工期**: 2周  
> **优先级**: 🔥 最高

---

## 📋 目录

- [1. 概述](#1-概述)
- [2. 技术设计](#2-技术设计)
- [3. 数据模型](#3-数据模型)
- [4. API 设计](#4-api-设计)
- [5. 实施步骤](#5-实施步骤)
- [6. 测试计划](#6-测试计划)

---

## 1. 概述

### 1.1 目标

创建 `flux-device` 包，实现完整的设备生命周期管理功能。

### 1.2 核心功能

- ✅ 设备注册与发现
- ✅ 设备认证与授权
- ✅ 设备分组管理
- ✅ 设备状态监控
- ✅ 设备元数据管理
- ✅ 设备生命周期管理

### 1.3 技术栈

- **语言**: Rust 1.75+
- **异步运行时**: Tokio
- **ORM**: SeaORM
- **数据库**: PostgreSQL
- **序列化**: Serde
- **日志**: Tracing

---

## 2. 技术设计

### 2.1 包结构

```
flux-device/
  ├── Cargo.toml
  ├── src/
  │   ├── lib.rs              # 模块导出
  │   ├── model.rs            # 数据模型
  │   ├── registry.rs         # 设备注册表
  │   ├── auth.rs             # 设备认证
  │   ├── group.rs            # 设备分组
  │   ├── monitor.rs          # 设备监控
  │   ├── manager.rs          # 设备管理器
  │   ├── error.rs            # 错误定义
  │   └── db/
  │       ├── mod.rs
  │       ├── entity.rs       # 数据库实体
  │       └── migration.rs    # 数据库迁移
  └── tests/
      ├── integration_test.rs
      └── fixtures/
```

### 2.2 核心组件

#### DeviceManager（设备管理器）
- 统一的设备管理入口
- 协调各个子模块
- 提供高层 API

#### DeviceRegistry（设备注册表）
- 设备注册/注销
- 设备查询
- 设备缓存

#### DeviceMonitor（设备监控）
- 心跳检测
- 状态追踪
- 健康检查

#### DeviceGroup（设备分组）
- 分组管理
- 层级结构
- 批量操作

---

## 3. 数据模型

### 3.1 设备模型（Device）

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// 设备 ID（全局唯一）
    pub id: String,
    
    /// 设备名称
    pub name: String,
    
    /// 设备类型
    pub device_type: DeviceType,
    
    /// 通信协议
    pub protocol: Protocol,
    
    /// 设备状态
    pub status: DeviceStatus,
    
    /// 产品 ID
    pub product_id: Option<String>,
    
    /// 设备密钥（加密存储）
    pub secret: Option<String>,
    
    /// 元数据（JSON）
    pub metadata: HashMap<String, String>,
    
    /// 标签
    pub tags: Vec<String>,
    
    /// 所属分组
    pub group_id: Option<String>,
    
    /// 地理位置
    pub location: Option<GeoLocation>,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    
    /// 最后在线时间
    pub last_seen: Option<DateTime<Utc>>,
}

/// 设备类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceType {
    /// 摄像头
    Camera,
    /// 传感器
    Sensor,
    /// 执行器
    Actuator,
    /// 网关
    Gateway,
    /// 工业设备
    Industrial,
    /// 智能家居
    SmartHome,
    /// 自定义类型
    Custom(String),
}

/// 通信协议
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Protocol {
    MQTT,
    CoAP,
    Modbus,
    OpcUa,
    HTTP,
    RTMP,
    RTSP,
    GB28181,
    ONVIF,
    Custom(String),
}

/// 设备状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceStatus {
    /// 在线
    Online,
    /// 离线
    Offline,
    /// 故障
    Fault,
    /// 维护中
    Maintenance,
    /// 未激活
    Inactive,
}

/// 地理位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub address: Option<String>,
}
```

### 3.2 设备分组模型（DeviceGroup）

```rust
/// 设备分组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceGroup {
    /// 分组 ID
    pub id: String,
    
    /// 分组名称
    pub name: String,
    
    /// 分组描述
    pub description: Option<String>,
    
    /// 父分组 ID（支持层级结构）
    pub parent_id: Option<String>,
    
    /// 分组路径（如：/root/building1/floor1）
    pub path: String,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}
```

### 3.3 设备状态历史（DeviceStatusHistory）

```rust
/// 设备状态历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatusHistory {
    pub id: i64,
    pub device_id: String,
    pub status: DeviceStatus,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<HashMap<String, String>>,
}
```

### 3.4 设备指标（DeviceMetrics）

```rust
/// 设备指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetrics {
    pub id: i64,
    pub device_id: String,
    pub metric_name: String,
    pub metric_value: f64,
    pub unit: Option<String>,
    pub timestamp: DateTime<Utc>,
}
```

---

## 4. API 设计

### 4.1 RESTful API

#### 设备管理

```
POST   /api/v1/devices              # 注册设备
GET    /api/v1/devices              # 列出设备（支持分页、过滤）
GET    /api/v1/devices/:id          # 获取设备详情
PUT    /api/v1/devices/:id          # 更新设备信息
DELETE /api/v1/devices/:id          # 删除设备
POST   /api/v1/devices/:id/activate # 激活设备
POST   /api/v1/devices/:id/deactivate # 停用设备
```

#### 设备状态

```
GET    /api/v1/devices/:id/status   # 获取设备状态
GET    /api/v1/devices/:id/metrics  # 获取设备指标
GET    /api/v1/devices/:id/history  # 获取状态历史
POST   /api/v1/devices/:id/heartbeat # 设备心跳
```

#### 设备分组

```
POST   /api/v1/device-groups        # 创建分组
GET    /api/v1/device-groups        # 列出分组
GET    /api/v1/device-groups/:id    # 获取分组详情
PUT    /api/v1/device-groups/:id    # 更新分组
DELETE /api/v1/device-groups/:id    # 删除分组
GET    /api/v1/device-groups/:id/devices # 获取分组下的设备
POST   /api/v1/device-groups/:id/devices/:device_id # 添加设备到分组
DELETE /api/v1/device-groups/:id/devices/:device_id # 从分组移除设备
```

### 4.2 请求/响应示例

#### 注册设备

**请求**：
```json
POST /api/v1/devices
{
  "name": "温度传感器-01",
  "device_type": "Sensor",
  "protocol": "MQTT",
  "product_id": "temp_sensor_v1",
  "metadata": {
    "model": "DHT22",
    "manufacturer": "ACME",
    "firmware_version": "1.0.0"
  },
  "tags": ["temperature", "humidity", "indoor"],
  "location": {
    "latitude": 39.9042,
    "longitude": 116.4074,
    "address": "北京市朝阳区"
  }
}
```

**响应**：
```json
{
  "id": "dev_1234567890",
  "name": "温度传感器-01",
  "device_type": "Sensor",
  "protocol": "MQTT",
  "status": "Inactive",
  "secret": "encrypted_secret_key",
  "created_at": "2026-02-22T15:30:00Z",
  "updated_at": "2026-02-22T15:30:00Z"
}
```

---

## 5. 实施步骤

### 第 1 天：创建包结构和基础模型

**任务**：
1. 创建 `flux-device` 包
2. 配置 `Cargo.toml` 依赖
3. 定义数据模型（`model.rs`）
4. 定义错误类型（`error.rs`）

**交付物**：
- ✅ 包结构创建完成
- ✅ 数据模型定义完成
- ✅ 编译通过

---

### 第 2-3 天：实现设备注册表

**任务**：
1. 实现 `DeviceRegistry`
2. 设备注册/注销功能
3. 设备查询功能
4. 设备缓存（Redis）

**代码示例**：
```rust
pub struct DeviceRegistry {
    db: Arc<DatabaseConnection>,
    cache: Arc<RwLock<HashMap<String, Device>>>,
}

impl DeviceRegistry {
    pub async fn register(&self, device: Device) -> Result<Device>;
    pub async fn unregister(&self, device_id: &str) -> Result<()>;
    pub async fn get(&self, device_id: &str) -> Result<Option<Device>>;
    pub async fn list(&self, filter: DeviceFilter) -> Result<Vec<Device>>;
    pub async fn update(&self, device_id: &str, device: Device) -> Result<Device>;
}
```

---

### 第 4-5 天：实现设备监控

**任务**：
1. 实现 `DeviceMonitor`
2. 心跳检测机制
3. 状态追踪
4. 健康检查

**代码示例**：
```rust
pub struct DeviceMonitor {
    registry: Arc<DeviceRegistry>,
    heartbeat_interval: Duration,
    timeout: Duration,
}

impl DeviceMonitor {
    pub async fn start(&self);
    pub async fn heartbeat(&self, device_id: &str) -> Result<()>;
    pub async fn check_status(&self, device_id: &str) -> Result<DeviceStatus>;
    pub async fn get_metrics(&self, device_id: &str) -> Result<Vec<DeviceMetrics>>;
}
```

---

### 第 6-7 天：实现设备分组

**任务**：
1. 实现 `DeviceGroup`
2. 分组 CRUD 操作
3. 层级结构支持
4. 设备分组关联

**代码示例**：
```rust
pub struct DeviceGroupManager {
    db: Arc<DatabaseConnection>,
}

impl DeviceGroupManager {
    pub async fn create_group(&self, group: DeviceGroup) -> Result<DeviceGroup>;
    pub async fn get_group(&self, group_id: &str) -> Result<Option<DeviceGroup>>;
    pub async fn list_groups(&self) -> Result<Vec<DeviceGroup>>;
    pub async fn add_device(&self, group_id: &str, device_id: &str) -> Result<()>;
    pub async fn remove_device(&self, group_id: &str, device_id: &str) -> Result<()>;
    pub async fn get_devices(&self, group_id: &str) -> Result<Vec<Device>>;
}
```

---

### 第 8-9 天：数据库设计和迁移

**任务**：
1. 设计数据库表结构
2. 创建 SeaORM 实体
3. 编写数据库迁移
4. 测试数据库操作

**表结构**：
```sql
-- 设备表
CREATE TABLE devices (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    device_type VARCHAR(50) NOT NULL,
    protocol VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'Inactive',
    product_id VARCHAR(64),
    secret TEXT,
    metadata JSONB,
    tags TEXT[],
    group_id VARCHAR(64),
    location JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMP,
    FOREIGN KEY (group_id) REFERENCES device_groups(id) ON DELETE SET NULL
);

CREATE INDEX idx_devices_status ON devices(status);
CREATE INDEX idx_devices_type ON devices(device_type);
CREATE INDEX idx_devices_group ON devices(group_id);
CREATE INDEX idx_devices_tags ON devices USING GIN(tags);

-- 设备分组表
CREATE TABLE device_groups (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    parent_id VARCHAR(64),
    path VARCHAR(1024) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    FOREIGN KEY (parent_id) REFERENCES device_groups(id) ON DELETE CASCADE
);

CREATE INDEX idx_groups_parent ON device_groups(parent_id);
CREATE INDEX idx_groups_path ON device_groups(path);

-- 设备状态历史表
CREATE TABLE device_status_history (
    id BIGSERIAL PRIMARY KEY,
    device_id VARCHAR(64) NOT NULL,
    status VARCHAR(20) NOT NULL,
    timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
    metadata JSONB,
    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE INDEX idx_status_history_device ON device_status_history(device_id);
CREATE INDEX idx_status_history_timestamp ON device_status_history(timestamp DESC);

-- 设备指标表（时序数据，后续迁移到 InfluxDB）
CREATE TABLE device_metrics (
    id BIGSERIAL PRIMARY KEY,
    device_id VARCHAR(64) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    unit VARCHAR(20),
    timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE INDEX idx_metrics_device ON device_metrics(device_id);
CREATE INDEX idx_metrics_name ON device_metrics(metric_name);
CREATE INDEX idx_metrics_timestamp ON device_metrics(timestamp DESC);
```

---

### 第 10 天：实现设备管理器

**任务**：
1. 实现 `DeviceManager`
2. 整合各个子模块
3. 提供统一 API

**代码示例**：
```rust
pub struct DeviceManager {
    registry: Arc<DeviceRegistry>,
    monitor: Arc<DeviceMonitor>,
    group_manager: Arc<DeviceGroupManager>,
}

impl DeviceManager {
    pub fn new(db: Arc<DatabaseConnection>) -> Self;
    
    // 设备管理
    pub async fn register_device(&self, device: Device) -> Result<Device>;
    pub async fn get_device(&self, device_id: &str) -> Result<Option<Device>>;
    pub async fn list_devices(&self, filter: DeviceFilter) -> Result<Vec<Device>>;
    pub async fn update_device(&self, device_id: &str, device: Device) -> Result<Device>;
    pub async fn delete_device(&self, device_id: &str) -> Result<()>;
    
    // 设备状态
    pub async fn heartbeat(&self, device_id: &str) -> Result<()>;
    pub async fn get_status(&self, device_id: &str) -> Result<DeviceStatus>;
    pub async fn get_metrics(&self, device_id: &str) -> Result<Vec<DeviceMetrics>>;
    
    // 设备分组
    pub async fn create_group(&self, group: DeviceGroup) -> Result<DeviceGroup>;
    pub async fn add_to_group(&self, group_id: &str, device_id: &str) -> Result<()>;
}
```

---

### 第 11-12 天：编写测试

**任务**：
1. 单元测试
2. 集成测试
3. 性能测试

**测试覆盖**：
- 设备注册/注销
- 设备查询
- 设备状态更新
- 设备分组操作
- 心跳检测
- 并发操作

---

### 第 13-14 天：集成和文档

**任务**：
1. 集成到主项目
2. 编写 API 文档
3. 编写使用示例
4. 代码审查和优化

---

## 6. 测试计划

### 6.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_device() {
        let manager = DeviceManager::new_test();
        let device = Device {
            id: "test_device_01".to_string(),
            name: "测试设备".to_string(),
            device_type: DeviceType::Sensor,
            protocol: Protocol::MQTT,
            status: DeviceStatus::Inactive,
            // ...
        };
        
        let result = manager.register_device(device).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_heartbeat() {
        let manager = DeviceManager::new_test();
        // 注册设备
        // 发送心跳
        // 验证状态变为 Online
    }
}
```

### 6.2 集成测试

- 数据库操作测试
- 缓存一致性测试
- 并发操作测试
- 性能基准测试

### 6.3 验收标准

- ✅ 单元测试覆盖率 > 80%
- ✅ 所有集成测试通过
- ✅ 支持 1000+ 设备注册
- ✅ 心跳检测延迟 < 100ms
- ✅ 设备查询响应 < 50ms

---

## 7. 交付清单

- [ ] flux-device 包代码
- [ ] 数据库迁移脚本
- [ ] API 文档
- [ ] 使用示例
- [ ] 测试代码
- [ ] README 文档

---

**维护者**: FLUX IOT Team  
**最后更新**: 2026-02-22
