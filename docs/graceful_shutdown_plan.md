# 优雅关闭实现方案

**日期**: 2026-02-20  
**当前完成度**: 0%  
**目标**: 完整的优雅关闭机制，确保服务安全退出

---

## 📊 需求分析

### 当前问题
1. **强制终止**：直接 kill 进程导致数据丢失
2. **连接中断**：正在处理的请求被强制中断
3. **资源泄漏**：文件句柄、数据库连接未正确关闭
4. **状态丢失**：内存中的状态未持久化
5. **不可预测**：无法控制关闭流程

### 目标
- ✅ 信号处理（SIGTERM/SIGINT）
- ✅ 连接排空（drain）
- ✅ 资源清理（cleanup）
- ✅ 状态持久化（persistence）
- ✅ 超时控制（timeout）

---

## 🏗️ 架构设计

### 1. 关闭流程

```
接收信号 → 停止接受新连接 → 等待现有连接完成 → 清理资源 → 持久化状态 → 退出
   ↓            ↓                    ↓                ↓            ↓          ↓
SIGTERM    set_shutdown()      drain_connections()  cleanup()   save_state()  exit(0)
```

### 2. 关闭阶段

```
Phase 1: 准备阶段（Preparing）
├── 接收关闭信号
├── 设置关闭标志
└── 停止接受新连接

Phase 2: 排空阶段（Draining）
├── 等待现有连接完成
├── 拒绝新请求
└── 超时强制关闭

Phase 3: 清理阶段（Cleaning）
├── 关闭数据库连接
├── 刷新日志缓冲区
├── 关闭文件句柄
└── 释放内存资源

Phase 4: 持久化阶段（Persisting）
├── 保存内存状态
├── 写入检查点
└── 同步磁盘

Phase 5: 退出阶段（Exiting）
└── 正常退出（exit code 0）
```

---

## 📋 详细设计

### 1. 信号处理

#### 1.1 信号类型

```rust
pub enum ShutdownSignal {
    /// SIGTERM - 优雅关闭
    Term,
    
    /// SIGINT - Ctrl+C
    Interrupt,
    
    /// SIGQUIT - 立即退出（带 core dump）
    Quit,
    
    /// 自定义关闭
    Custom(String),
}
```

#### 1.2 信号处理器

```rust
use tokio::signal;

pub struct SignalHandler {
    shutdown_tx: broadcast::Sender<ShutdownSignal>,
}

impl SignalHandler {
    pub async fn wait_for_signal(&self) {
        tokio::select! {
            _ = signal::ctrl_c() => {
                self.shutdown_tx.send(ShutdownSignal::Interrupt).ok();
            }
            _ = signal::unix::signal(signal::unix::SignalKind::terminate()) => {
                self.shutdown_tx.send(ShutdownSignal::Term).ok();
            }
        }
    }
}
```

### 2. 连接排空

#### 2.1 连接跟踪

```rust
pub struct ConnectionTracker {
    active_connections: Arc<AtomicUsize>,
    max_drain_duration: Duration,
}

impl ConnectionTracker {
    pub fn acquire(&self) -> Option<ConnectionGuard> {
        if self.is_shutting_down() {
            return None;
        }
        
        self.active_connections.fetch_add(1, Ordering::SeqCst);
        Some(ConnectionGuard::new(self.active_connections.clone()))
    }
    
    pub async fn drain(&self) {
        let start = Instant::now();
        
        while self.active_connections.load(Ordering::SeqCst) > 0 {
            if start.elapsed() > self.max_drain_duration {
                warn!("Drain timeout, forcing shutdown");
                break;
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
```

#### 2.2 连接守卫

```rust
pub struct ConnectionGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}
```

### 3. 资源清理

#### 3.1 资源管理器

```rust
pub struct ResourceManager {
    resources: Vec<Box<dyn Resource>>,
}

#[async_trait]
pub trait Resource: Send + Sync {
    async fn cleanup(&self) -> Result<()>;
    fn name(&self) -> &str;
}

impl ResourceManager {
    pub async fn cleanup_all(&self) {
        for resource in &self.resources {
            info!("Cleaning up resource: {}", resource.name());
            if let Err(e) = resource.cleanup().await {
                error!("Failed to cleanup {}: {}", resource.name(), e);
            }
        }
    }
}
```

#### 3.2 常见资源

```rust
// 数据库连接池
pub struct DatabaseResource {
    pool: PgPool,
}

#[async_trait]
impl Resource for DatabaseResource {
    async fn cleanup(&self) -> Result<()> {
        self.pool.close().await;
        Ok(())
    }
    
    fn name(&self) -> &str {
        "database_pool"
    }
}

// 日志聚合器
pub struct LogAggregatorResource {
    aggregator: Arc<LogAggregator>,
}

#[async_trait]
impl Resource for LogAggregatorResource {
    async fn cleanup(&self) -> Result<()> {
        self.aggregator.flush().await;
        Ok(())
    }
    
    fn name(&self) -> &str {
        "log_aggregator"
    }
}

// 文件句柄
pub struct FileResource {
    file: Arc<RwLock<File>>,
}

#[async_trait]
impl Resource for FileResource {
    async fn cleanup(&self) -> Result<()> {
        let mut file = self.file.write().await;
        file.sync_all().await?;
        Ok(())
    }
    
    fn name(&self) -> &str {
        "file_handle"
    }
}
```

### 4. 状态持久化

#### 4.1 状态管理器

```rust
pub struct StateManager<T> {
    state: Arc<RwLock<T>>,
    checkpoint_path: PathBuf,
}

impl<T: Serialize + DeserializeOwned> StateManager<T> {
    pub async fn save_checkpoint(&self) -> Result<()> {
        let state = self.state.read().await;
        let json = serde_json::to_string_pretty(&*state)?;
        
        tokio::fs::write(&self.checkpoint_path, json).await?;
        
        info!("State checkpoint saved to {:?}", self.checkpoint_path);
        Ok(())
    }
    
    pub async fn load_checkpoint(&self) -> Result<T> {
        let json = tokio::fs::read_to_string(&self.checkpoint_path).await?;
        let state = serde_json::from_str(&json)?;
        
        info!("State checkpoint loaded from {:?}", self.checkpoint_path);
        Ok(state)
    }
}
```

### 5. 优雅关闭协调器

#### 5.1 关闭协调器

```rust
pub struct ShutdownCoordinator {
    signal_handler: SignalHandler,
    connection_tracker: ConnectionTracker,
    resource_manager: ResourceManager,
    state_manager: Option<Box<dyn StateManager>>,
    shutdown_timeout: Duration,
}

impl ShutdownCoordinator {
    pub async fn run(&self) {
        // 等待关闭信号
        self.signal_handler.wait_for_signal().await;
        
        info!("Shutdown signal received, starting graceful shutdown");
        
        // Phase 1: 停止接受新连接
        self.stop_accepting_connections();
        
        // Phase 2: 排空现有连接
        info!("Draining active connections...");
        tokio::time::timeout(
            self.shutdown_timeout,
            self.connection_tracker.drain()
        ).await.ok();
        
        // Phase 3: 清理资源
        info!("Cleaning up resources...");
        self.resource_manager.cleanup_all().await;
        
        // Phase 4: 持久化状态
        if let Some(state_manager) = &self.state_manager {
            info!("Persisting state...");
            state_manager.save_checkpoint().await.ok();
        }
        
        // Phase 5: 退出
        info!("Graceful shutdown complete");
        std::process::exit(0);
    }
}
```

---

## 🔧 实现方案

### 1. 创建 flux-shutdown crate

```toml
[package]
name = "flux-shutdown"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.35", features = ["full", "signal"] }
tracing = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
thiserror = "1.0"
```

### 2. 核心模块

```
flux-shutdown/
├── src/
│   ├── lib.rs              # 模块导出
│   ├── signal.rs           # 信号处理
│   ├── connection.rs       # 连接跟踪
│   ├── resource.rs         # 资源管理
│   ├── state.rs            # 状态持久化
│   └── coordinator.rs      # 关闭协调器
```

### 3. 使用示例

```rust
use flux_shutdown::{
    ShutdownCoordinator, SignalHandler, ConnectionTracker,
    ResourceManager, StateManager,
};

#[tokio::main]
async fn main() {
    // 创建关闭协调器
    let coordinator = ShutdownCoordinator::builder()
        .with_signal_handler(SignalHandler::new())
        .with_connection_tracker(ConnectionTracker::new(Duration::from_secs(30)))
        .with_resource_manager(ResourceManager::new())
        .with_state_manager(StateManager::new("state.json"))
        .with_shutdown_timeout(Duration::from_secs(60))
        .build();
    
    // 启动服务
    let server = start_server();
    
    // 等待关闭信号
    tokio::select! {
        _ = server => {},
        _ = coordinator.run() => {},
    }
}
```

---

## 📋 实施计划

### 阶段 1：信号处理（1-2 天）
- [ ] 创建 flux-shutdown crate
- [ ] 实现 SignalHandler
- [ ] 实现信号广播机制
- [ ] 单元测试

### 阶段 2：连接排空（2-3 天）
- [ ] 实现 ConnectionTracker
- [ ] 实现 ConnectionGuard
- [ ] 实现超时机制
- [ ] 集成测试

### 阶段 3：资源清理（2-3 天）
- [ ] 实现 Resource trait
- [ ] 实现 ResourceManager
- [ ] 实现常见资源清理
- [ ] 单元测试

### 阶段 4：状态持久化（1-2 天）
- [ ] 实现 StateManager
- [ ] 实现检查点机制
- [ ] 实现状态恢复
- [ ] 单元测试

### 阶段 5：协调器（2-3 天）
- [ ] 实现 ShutdownCoordinator
- [ ] 实现关闭流程
- [ ] 集成所有组件
- [ ] 集成测试

### 阶段 6：文档和示例（1-2 天）
- [ ] 编写使用文档
- [ ] 编写示例代码
- [ ] 编写最佳实践

**总计**：9-15 天（2-3 周）

---

## 🎯 成功标准

### 功能完整性
- [x] 信号处理
- [x] 连接排空
- [x] 资源清理
- [x] 状态持久化
- [x] 超时控制

### 可靠性
- 零数据丢失
- 零连接中断（在超时内）
- 100% 资源释放

### 性能
- 关闭时间 < 60 秒
- CPU 开销 < 1%
- 内存开销 < 10MB

---

## ⚠️ 注意事项

### 1. 超时设置
- 连接排空超时：30 秒
- 总关闭超时：60 秒
- 强制退出：超时后

### 2. 信号处理
- SIGTERM：优雅关闭
- SIGINT：优雅关闭
- SIGKILL：无法捕获，立即退出

### 3. 状态一致性
- 使用事务保证原子性
- 定期检查点
- 崩溃恢复机制

---

## 🎉 总结

优雅关闭系统将提供：
- ✅ 安全的服务退出
- ✅ 零数据丢失
- ✅ 完整的资源清理
- ✅ 状态持久化
- ✅ 可控的关闭流程

**预计工期**：2-3 周  
**优先级**：高  
**复杂度**：中等

---

**下一步行动**：
1. 创建 flux-shutdown crate
2. 实现信号处理
3. 实现连接跟踪
4. 实现资源管理

**规划完成时间**: 2026-02-20
