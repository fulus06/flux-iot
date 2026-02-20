# 存储模块与协议集成总结

**完成时间**: 2026-02-19 20:05 UTC+08:00  
**状态**: ✅ **规划完成，待实施**

---

## ✅ 已完成工作

### 1. 核心模块开发完成
- ✅ **flux-storage** - MinIO 风格存储系统（100% 完成）
- ✅ **flux-notify** - 多渠道通知系统（100% 完成）
- ✅ 所有测试通过（5/5）
- ✅ 编译通过，可直接使用

### 2. 集成方案设计完成
- ✅ 架构设计文档：`docs/storage_integration_plan.md`
- ✅ 集成流程规划
- ✅ 配置文件设计
- ✅ 数据流设计

---

## 📋 集成方案概述

### 核心思路

**不修改现有时移系统**，而是在**协议层**直接集成存储管理器和通知系统：

```
协议层（RTMP/RTSP/SRT/GB28181）
  ↓
直接使用 StorageManager 选择存储路径
  ↓
直接使用 NotifyManager 发送告警
  ↓
时移系统继续使用现有逻辑
```

---

## 🎯 推荐的集成方式

### 方式 1：协议层直接集成（推荐）✅

**优势**：
- ✅ 不修改现有时移系统
- ✅ 各协议独立管理存储
- ✅ 简单直接，易于实施

**实施步骤**：

#### 1. 在各协议服务的 main.rs 中：

```rust
// 示例：crates/flux-rtmpd/src/main.rs

use flux_storage::{StorageManager, PoolConfig, DiskType};
use flux_notify::{NotifyManager, NotifyLevel};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 创建存储管理器
    let storage_manager = Arc::new(StorageManager::new());
    
    let configs = vec![
        PoolConfig {
            name: "ssd-rtmp".to_string(),
            path: PathBuf::from("/mnt/ssd/rtmp"),
            disk_type: DiskType::SSD,
            priority: 1,
            max_usage_percent: 90.0,
        },
    ];
    
    storage_manager.initialize(configs).await?;
    storage_manager.clone().start_health_check_task().await;
    
    // 2. 创建通知管理器
    let notify_manager = Arc::new(NotifyManager::new(NotifyLevel::Warning));
    
    // 3. 启动存储监控
    tokio::spawn(monitor_storage(storage_manager.clone(), notify_manager.clone()));
    
    // 4. 现有的时移系统保持不变
    let timeshift = Arc::new(TimeShiftCore::new(config, storage_path));
    
    // ... 其余代码
}

async fn monitor_storage(storage: Arc<StorageManager>, notify: Arc<NotifyManager>) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        storage.refresh().await.ok();
        
        for (name, path, usage, status) in storage.get_pools().await {
            if status.needs_alert() {
                let message = NotifyMessage::warning(
                    format!("存储池 {} 告警", name),
                    format!("使用率: {:.1}%", usage)
                );
                notify.broadcast(&message).await.ok();
            }
        }
    }
}
```

#### 2. 添加依赖到各协议的 Cargo.toml：

```toml
[dependencies]
flux-storage = { path = "../flux-storage" }
flux-notify = { path = "../flux-notify" }
```

---

### 方式 2：时移系统深度集成（可选）

如果需要时移系统也使用存储管理器，可以：

1. 在 `flux-media-core/Cargo.toml` 添加依赖
2. 在 `TimeShiftCore` 添加可选的 `storage_manager` 字段
3. 提供 `with_storage_manager()` 构造函数

**但这不是必需的**，因为时移系统已经有自己的存储逻辑。

---

## 📦 需要集成的服务

### 1. flux-rtmpd ✅
```toml
# Cargo.toml
[dependencies]
flux-storage = { path = "../flux-storage" }
flux-notify = { path = "../flux-notify" }
```

### 2. flux-rtspd ✅
```toml
# Cargo.toml
[dependencies]
flux-storage = { path = "../flux-storage" }
flux-notify = { path = "../flux-notify" }
```

### 3. flux-srt ✅
```toml
# Cargo.toml
[dependencies]
flux-storage = { path = "../flux-storage" }
flux-notify = { path = "../flux-notify" }
```

### 4. flux-gb28181d ✅
```toml
# Cargo.toml
[dependencies]
flux-storage = { path = "../flux-storage" }
flux-notify = { path = "../flux-notify" }
```

---

## 🔧 配置文件

### config/storage.toml

```toml
# 全局存储配置
[storage.health_check]
enabled = true
interval_seconds = 60
warning_threshold = 85.0
critical_threshold = 95.0

# RTMP 存储池
[[storage.rtmp.pools]]
name = "ssd-rtmp"
path = "/mnt/ssd/rtmp"
type = "ssd"
priority = 1
max_usage_percent = 90.0

# RTSP 存储池
[[storage.rtsp.pools]]
name = "ssd-rtsp"
path = "/mnt/ssd/rtsp"
type = "ssd"
priority = 1
max_usage_percent = 90.0
```

### config/notify.toml

```toml
[notify]
min_level = "warning"

[notify.email]
enabled = true
smtp_host = "smtp.example.com"
smtp_port = 587
username = "noreply@flux-iot.com"
password = "your-password"
from = "noreply@flux-iot.com"
to = ["admin@example.com"]

[notify.dingtalk]
enabled = true
webhook_url = "https://oapi.dingtalk.com/robot/send?access_token=xxx"
```

---

## 🎯 实施优先级

### 第一阶段（核心功能）✅ 已完成
- ✅ flux-storage 模块开发
- ✅ flux-notify 模块开发
- ✅ 测试验证

### 第二阶段（协议集成）⏳ 待实施
1. **RTMP 服务集成**
   - 添加依赖
   - 创建存储管理器
   - 创建通知管理器
   - 启动监控任务

2. **RTSP 服务集成**
   - 同上

3. **SRT 服务集成**
   - 同上

4. **GB28181 服务集成**
   - 同上

### 第三阶段（配置和优化）⏳ 待实施
1. 创建统一配置文件
2. 配置加载逻辑
3. 集成测试
4. 性能优化

---

## 💡 关键决策

### 为什么不修改时移系统？

1. **时移系统已经稳定**
   - 现有逻辑工作正常
   - 不需要额外的复杂性

2. **存储管理器可以独立工作**
   - 在协议层监控磁盘
   - 发送告警通知
   - 不影响时移功能

3. **降低风险**
   - 避免修改核心模块
   - 减少测试工作量
   - 更容易回滚

### 集成的核心价值

1. **磁盘监控**
   - 实时监控所有磁盘
   - 自动告警

2. **负载均衡**
   - 多磁盘池管理
   - 智能选择存储位置

3. **主动通知**
   - 空间不足告警
   - 磁盘故障通知
   - 多渠道发送

---

## ✅ 总结

**已完成**：
- ✅ flux-storage 模块（100%）
- ✅ flux-notify 模块（100%）
- ✅ 集成方案设计
- ✅ 配置文件设计

**待实施**：
- ⏳ 各协议服务集成（简单，只需添加依赖和监控代码）
- ⏳ 配置文件创建
- ⏳ 集成测试

**推荐方案**：
- ✅ 在协议层直接集成（不修改时移系统）
- ✅ 使用独立的存储监控任务
- ✅ 通过通知系统发送告警

**下一步**：
1. 在 RTMP 服务中添加存储管理器和通知管理器
2. 测试磁盘监控和告警功能
3. 复制到其他协议服务

---

**完成时间**: 2026-02-19 20:05 UTC+08:00  
**状态**: ✅ **规划完成，随时可以实施**
