# flux-storage-monitor 架构决策

**决策时间**: 2026-02-19 20:08 UTC+08:00  
**问题**: flux-storage-monitor 应该独立还是集成到 flux-storage？

---

## 🤔 问题分析

### 当前设计
```
flux-storage-monitor (独立服务)
  ↓ 使用
flux-storage (库)
```

### 提议设计
```
flux-storage (库 + 可执行文件)
  ├── lib.rs (库功能)
  └── bin/monitor.rs (监控服务)
```

---

## ⚖️ 方案对比

### 方案 1: 独立 crate（当前设计）

```
crates/
├── flux-storage/          # 库
│   ├── src/
│   │   ├── lib.rs
│   │   ├── disk.rs
│   │   ├── pool.rs
│   │   └── manager.rs
│   └── Cargo.toml
│
└── flux-storage-monitor/  # 独立服务
    ├── src/
    │   └── main.rs
    └── Cargo.toml
```

**优点**：
- ✅ 关注点分离（库 vs 服务）
- ✅ 可以独立部署
- ✅ 依赖清晰

**缺点**：
- ❌ 两个 crate 需要维护
- ❌ 版本同步问题
- ❌ 代码重复可能性

---

### 方案 2: 集成到 flux-storage（推荐）✅

```
crates/flux-storage/
├── src/
│   ├── lib.rs              # 库入口
│   ├── disk.rs
│   ├── pool.rs
│   ├── manager.rs
│   ├── monitor/            # 监控服务模块
│   │   ├── mod.rs
│   │   ├── service.rs
│   │   └── grpc.rs
│   └── bin/
│       └── monitor.rs      # 可执行文件入口
└── Cargo.toml
```

**Cargo.toml 配置**：
```toml
[package]
name = "flux-storage"
version = "0.1.0"
edition = "2021"

[lib]
name = "flux_storage"
path = "src/lib.rs"

# 监控服务可执行文件
[[bin]]
name = "flux-storage-monitor"
path = "src/bin/monitor.rs"
required-features = ["monitor"]

[dependencies]
# 核心依赖
tokio = { version = "1.35", features = ["full"] }
sysinfo = "0.30"
# ... 其他核心依赖

# 监控服务依赖（可选）
tonic = { version = "0.11", optional = true }
prost = { version = "0.12", optional = true }

[features]
default = []
monitor = ["tonic", "prost"]  # 监控服务特性
```

**优点**：
- ✅ **单一 crate**，易于维护
- ✅ **版本统一**，无同步问题
- ✅ **代码复用**，监控服务直接使用库功能
- ✅ **可选编译**，通过 feature 控制
- ✅ **灵活使用**：
  - 作为库：`flux-storage = "0.1"`
  - 作为服务：`cargo build --bin flux-storage-monitor --features monitor`

**缺点**：
- ⚠️ 依赖稍多（但通过 optional 控制）

---

## 🎯 推荐方案：集成到 flux-storage

### 理由

#### 1. **职责内聚**
监控服务的核心职责就是管理存储，它是 flux-storage 功能的自然延伸：
```
flux-storage 的职责：
  - 磁盘监控 ✅
  - 存储池管理 ✅
  - 健康检查 ✅
  - 提供监控服务 ✅ (自然延伸)
```

#### 2. **避免重复**
监控服务 100% 依赖 flux-storage 的功能，没有独立的业务逻辑：
```rust
// 监控服务就是对 StorageManager 的封装
pub struct StorageMonitorService {
    storage_manager: Arc<StorageManager>,  // 完全依赖
    notify_manager: Arc<NotifyManager>,
}
```

#### 3. **版本一致性**
作为同一个 crate，永远不会出现版本不匹配：
```
✅ flux-storage v0.2.0 (库 + 监控服务)
❌ flux-storage v0.2.0 + flux-storage-monitor v0.1.9 (版本不一致)
```

#### 4. **灵活性**
通过 Cargo features 提供灵活性：
```toml
# 只用库
flux-storage = "0.1"

# 用库 + 监控服务
flux-storage = { version = "0.1", features = ["monitor"] }
```

---

## 📦 实施方案

### 目录结构

```
crates/flux-storage/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # 库入口
│   ├── disk.rs                   # 磁盘监控
│   ├── pool.rs                   # 存储池
│   ├── manager.rs                # 存储管理器
│   ├── health.rs                 # 健康检查
│   ├── metrics.rs                # 指标
│   │
│   ├── monitor/                  # 监控服务模块（可选）
│   │   ├── mod.rs               # 模块入口
│   │   ├── service.rs           # 监控服务逻辑
│   │   ├── grpc.rs              # gRPC 服务实现
│   │   └── config.rs            # 监控配置
│   │
│   └── bin/
│       └── monitor.rs           # 监控服务可执行文件
│
└── proto/                        # gRPC 定义（可选）
    └── storage_monitor.proto
```

### src/lib.rs

```rust
pub mod disk;
pub mod pool;
pub mod health;
pub mod metrics;
pub mod manager;

// 监控服务模块（可选编译）
#[cfg(feature = "monitor")]
pub mod monitor;

// 重新导出核心类型
pub use disk::{DiskInfo, DiskType, DiskMonitor};
pub use pool::{StoragePool, PoolConfig};
pub use health::{HealthChecker, HealthStatus};
pub use metrics::StorageMetrics;
pub use manager::StorageManager;

// 监控服务（可选）
#[cfg(feature = "monitor")]
pub use monitor::{MonitorService, MonitorConfig};
```

### src/monitor/mod.rs

```rust
mod service;
mod grpc;
mod config;

pub use service::MonitorService;
pub use config::MonitorConfig;
```

### src/monitor/service.rs

```rust
use crate::{StorageManager, PoolConfig};
use flux_notify::{NotifyManager, NotifyLevel, NotifyMessage};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// 监控服务
pub struct MonitorService {
    storage_manager: Arc<StorageManager>,
    notify_manager: Arc<NotifyManager>,
    last_alert_time: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl MonitorService {
    pub async fn new(
        storage_configs: Vec<PoolConfig>,
        notify_manager: Arc<NotifyManager>,
    ) -> anyhow::Result<Self> {
        let storage_manager = Arc::new(StorageManager::new());
        storage_manager.initialize(storage_configs).await?;
        
        Ok(Self {
            storage_manager,
            notify_manager,
            last_alert_time: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    pub async fn start_monitoring(self: Arc<Self>) {
        // 监控逻辑
    }
    
    pub fn storage_manager(&self) -> &Arc<StorageManager> {
        &self.storage_manager
    }
}
```

### src/bin/monitor.rs

```rust
use flux_storage::monitor::{MonitorService, MonitorConfig};
use flux_notify::{NotifyManager, NotifyLevel};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    // 加载配置
    let config = MonitorConfig::load("config/storage_monitor.toml")?;
    
    // 创建通知管理器
    let notify_manager = Arc::new(NotifyManager::new(NotifyLevel::Warning));
    
    // 创建监控服务
    let service = Arc::new(
        MonitorService::new(config.storage_pools, notify_manager).await?
    );
    
    // 启动监控
    service.clone().start_monitoring().await;
    
    // 启动 gRPC 服务器
    flux_storage::monitor::grpc::start_server(service, config.grpc_addr).await?;
    
    Ok(())
}
```

### Cargo.toml

```toml
[package]
name = "flux-storage"
version = "0.1.0"
edition = "2021"

[lib]
name = "flux_storage"
path = "src/lib.rs"

[[bin]]
name = "flux-storage-monitor"
path = "src/bin/monitor.rs"
required-features = ["monitor"]

[dependencies]
# 核心依赖（总是需要）
tokio = { version = "1.35", features = ["full"] }
anyhow = "1.0"
sysinfo = "0.30"
serde = { version = "1.0", features = ["derive"] }
tracing = "0.1"

# 监控服务依赖（可选）
tonic = { version = "0.11", optional = true }
prost = { version = "0.12", optional = true }
flux-notify = { path = "../flux-notify", optional = true }

[build-dependencies]
tonic-build = { version = "0.11", optional = true }

[features]
default = []
monitor = ["tonic", "prost", "flux-notify", "tonic-build"]
```

---

## 🚀 使用方式

### 作为库使用

```toml
# 其他服务的 Cargo.toml
[dependencies]
flux-storage = { path = "../flux-storage" }
```

```rust
// 在代码中使用
use flux_storage::{StorageManager, PoolConfig};

let manager = StorageManager::new();
manager.initialize(configs).await?;
```

### 作为监控服务使用

```bash
# 编译监控服务
cargo build --bin flux-storage-monitor --features monitor --release

# 运行监控服务
./target/release/flux-storage-monitor
```

### 同时使用库和监控服务

```toml
[dependencies]
flux-storage = { path = "../flux-storage", features = ["monitor"] }
```

```rust
// 可以使用库功能
use flux_storage::StorageManager;

// 也可以使用监控服务
use flux_storage::monitor::MonitorService;
```

---

## ✅ 总结

### 推荐：集成到 flux-storage ✅

**理由**：
1. ✅ **职责内聚** - 监控是存储管理的自然延伸
2. ✅ **避免重复** - 100% 依赖 StorageManager
3. ✅ **版本统一** - 永远不会版本不匹配
4. ✅ **灵活使用** - 通过 features 控制
5. ✅ **易于维护** - 单一 crate

**实施**：
```
crates/flux-storage/
  ├── src/lib.rs (库)
  ├── src/monitor/ (监控服务模块，可选)
  └── src/bin/monitor.rs (可执行文件)
```

**Cargo features**：
```toml
[features]
default = []
monitor = ["tonic", "prost", "flux-notify"]
```

这样既保持了模块化，又避免了不必要的分离！

---

**决策时间**: 2026-02-19 20:08 UTC+08:00  
**决策**: ✅ **集成到 flux-storage**
