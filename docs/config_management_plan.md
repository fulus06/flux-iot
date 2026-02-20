# 配置管理方案

**日期**: 2026-02-20  
**当前完成度**: 0%  
**目标**: 统一的动态配置热更新系统

---

## 📊 需求分析

### 当前问题
1. **配置分散**：各服务独立加载配置，缺乏统一管理
2. **无热更新**：修改配置需要重启服务
3. **无版本控制**：配置变更无历史记录
4. **无校验机制**：错误配置可能导致服务崩溃
5. **无回滚能力**：配置错误后难以快速恢复

### 目标
- ✅ 统一的配置管理接口
- ✅ 动态热更新（无需重启）
- ✅ 多数据源支持（file/sqlite/postgres）
- ✅ 配置校验和冲突检测
- ✅ 配置版本管理和回滚
- ✅ 变更通知机制

---

## 🏗️ 架构设计

### 1. 核心组件

```
flux-config-manager/
├── src/
│   ├── lib.rs              # 库导出
│   ├── manager.rs          # ConfigManager 核心
│   ├── source.rs           # 配置源抽象
│   ├── watcher.rs          # 配置监听器
│   ├── validator.rs        # 配置校验器
│   ├── version.rs          # 版本管理
│   ├── notifier.rs         # 变更通知
│   └── sources/
│       ├── file.rs         # 文件源
│       ├── sqlite.rs       # SQLite 源
│       └── postgres.rs     # PostgreSQL 源
└── tests/
    └── integration_tests.rs
```

### 2. 数据流

```
配置源 → Watcher → Validator → Manager → Notifier → 服务
  ↓                                ↓
版本控制                         回滚支持
```

---

## 📋 详细设计

### 1. ConfigManager 核心

```rust
pub struct ConfigManager {
    // 配置源
    source: Arc<dyn ConfigSource>,
    // 当前配置
    current_config: Arc<RwLock<Config>>,
    // 配置历史
    history: Arc<RwLock<Vec<ConfigVersion>>>,
    // 变更通知器
    notifiers: Vec<Arc<dyn ConfigNotifier>>,
    // 监听器
    watcher: Option<ConfigWatcher>,
}

impl ConfigManager {
    pub async fn new(source: Arc<dyn ConfigSource>) -> Result<Self>;
    pub async fn load(&mut self) -> Result<Config>;
    pub async fn reload(&mut self) -> Result<()>;
    pub async fn update(&mut self, config: Config) -> Result<()>;
    pub async fn rollback(&mut self, version: u64) -> Result<()>;
    pub async fn validate(&self, config: &Config) -> Result<()>;
    pub fn subscribe(&mut self, notifier: Arc<dyn ConfigNotifier>);
}
```

### 2. ConfigSource 抽象

```rust
#[async_trait]
pub trait ConfigSource: Send + Sync {
    async fn load(&self) -> Result<Config>;
    async fn save(&self, config: &Config) -> Result<()>;
    async fn watch(&self) -> Result<ConfigWatcher>;
}

// 文件源
pub struct FileSource {
    path: PathBuf,
}

// SQLite 源
pub struct SqliteSource {
    pool: SqlitePool,
}

// PostgreSQL 源
pub struct PostgresSource {
    pool: PgPool,
}
```

### 3. 配置监听器

```rust
pub struct ConfigWatcher {
    rx: mpsc::Receiver<ConfigChange>,
}

pub enum ConfigChange {
    Updated(Config),
    Deleted,
}

impl ConfigWatcher {
    pub async fn watch(&mut self) -> Option<ConfigChange>;
}
```

### 4. 配置校验器

```rust
pub struct ConfigValidator {
    rules: Vec<Box<dyn ValidationRule>>,
}

pub trait ValidationRule: Send + Sync {
    fn validate(&self, config: &Config) -> Result<()>;
}

// 内置规则
pub struct RequiredFieldRule;
pub struct RangeRule;
pub struct FormatRule;
pub struct ConflictRule;
```

### 5. 版本管理

```rust
pub struct ConfigVersion {
    pub version: u64,
    pub config: Config,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub comment: String,
}

pub struct VersionManager {
    versions: Vec<ConfigVersion>,
    max_versions: usize,
}

impl VersionManager {
    pub fn add(&mut self, config: Config, author: String, comment: String);
    pub fn get(&self, version: u64) -> Option<&ConfigVersion>;
    pub fn list(&self) -> &[ConfigVersion];
    pub fn rollback(&mut self, version: u64) -> Result<Config>;
}
```

### 6. 变更通知

```rust
#[async_trait]
pub trait ConfigNotifier: Send + Sync {
    async fn notify(&self, old: &Config, new: &Config) -> Result<()>;
}

// 通道通知器
pub struct ChannelNotifier {
    tx: mpsc::Sender<ConfigChange>,
}

// HTTP 回调通知器
pub struct HttpCallbackNotifier {
    url: String,
}

// 日志通知器
pub struct LogNotifier;
```

---

## 🔧 配置格式

### TOML 格式

```toml
[service]
name = "flux-rtspd"
version = "1.0.0"

[rtsp]
bind = "0.0.0.0:8554"
max_connections = 100
timeout_ms = 5000

[storage]
root_dir = "/data/storage"
retention_days = 7

[timeshift]
enabled = true
hot_cache_duration = 3600
cold_storage_duration = 86400
```

### JSON 格式

```json
{
  "service": {
    "name": "flux-rtspd",
    "version": "1.0.0"
  },
  "rtsp": {
    "bind": "0.0.0.0:8554",
    "max_connections": 100,
    "timeout_ms": 5000
  }
}
```

---

## 📊 数据库 Schema

### SQLite

```sql
CREATE TABLE configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL,
    config_key TEXT NOT NULL,
    config_value TEXT NOT NULL,
    version INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    author TEXT,
    comment TEXT,
    UNIQUE(service_name, config_key, version)
);

CREATE INDEX idx_configs_service ON configs(service_name);
CREATE INDEX idx_configs_version ON configs(version);
```

### PostgreSQL

```sql
CREATE TABLE configs (
    id SERIAL PRIMARY KEY,
    service_name VARCHAR(255) NOT NULL,
    config_key VARCHAR(255) NOT NULL,
    config_value JSONB NOT NULL,
    version BIGINT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    author VARCHAR(255),
    comment TEXT,
    UNIQUE(service_name, config_key, version)
);

CREATE INDEX idx_configs_service ON configs(service_name);
CREATE INDEX idx_configs_version ON configs(version);
CREATE INDEX idx_configs_value ON configs USING GIN(config_value);
```

---

## 🚀 使用示例

### 基本使用

```rust
use flux_config_manager::{ConfigManager, FileSource};

#[tokio::main]
async fn main() -> Result<()> {
    // 创建文件源
    let source = Arc::new(FileSource::new("config.toml"));
    
    // 创建配置管理器
    let mut manager = ConfigManager::new(source).await?;
    
    // 加载配置
    let config = manager.load().await?;
    println!("Loaded config: {:?}", config);
    
    // 订阅变更通知
    let (tx, mut rx) = mpsc::channel(10);
    manager.subscribe(Arc::new(ChannelNotifier::new(tx)));
    
    // 监听配置变更
    tokio::spawn(async move {
        while let Some(change) = rx.recv().await {
            println!("Config changed: {:?}", change);
        }
    });
    
    // 启动热更新
    manager.start_watching().await?;
    
    Ok(())
}
```

### 配置更新

```rust
// 更新配置
let mut new_config = config.clone();
new_config.rtsp.max_connections = 200;

// 校验
manager.validate(&new_config).await?;

// 应用
manager.update(new_config).await?;
```

### 配置回滚

```rust
// 查看历史版本
let versions = manager.list_versions().await?;
for v in versions {
    println!("Version {}: {}", v.version, v.comment);
}

// 回滚到指定版本
manager.rollback(5).await?;
```

---

## 📋 实施计划

### 阶段 1：核心框架（3-4 天）
- [ ] 创建 flux-config-manager crate
- [ ] 实现 ConfigManager 核心
- [ ] 实现 ConfigSource trait
- [ ] 实现 FileSource
- [ ] 基本的加载/保存功能

### 阶段 2：热更新机制（2-3 天）
- [ ] 实现 ConfigWatcher
- [ ] 文件监听（notify crate）
- [ ] 变更检测和通知
- [ ] 自动重载逻辑

### 阶段 3：数据库支持（3-4 天）
- [ ] 实现 SqliteSource
- [ ] 实现 PostgresSource
- [ ] 数据库 schema 和迁移
- [ ] 连接池管理

### 阶段 4：校验和版本（2-3 天）
- [ ] 实现 ConfigValidator
- [ ] 内置校验规则
- [ ] 版本管理
- [ ] 回滚功能

### 阶段 5：集成和测试（2-3 天）
- [ ] 集成到现有服务
- [ ] 单元测试
- [ ] 集成测试
- [ ] 文档和示例

**总计**：12-17 天（2-3 周）

---

## 🎯 成功标准

### 功能完整性
- [x] 支持多种配置源
- [x] 热更新无需重启
- [x] 配置校验
- [x] 版本管理和回滚
- [x] 变更通知

### 性能指标
- 配置加载：< 100ms
- 热更新延迟：< 1s
- 内存占用：< 10MB

### 可靠性
- 配置错误不影响服务运行
- 自动回滚到上一个有效配置
- 完整的错误日志

---

## 📚 依赖库

```toml
[dependencies]
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
async-trait = "0.1"

# 文件监听
notify = "6.1"

# 数据库
sqlx = { version = "0.7", features = ["sqlite", "postgres", "runtime-tokio-rustls"] }

# 时间
chrono = { version = "0.4", features = ["serde"] }
```

---

## 🔄 与现有系统集成

### flux-config 升级

现有的 `flux-config` crate 将升级为 `flux-config-manager`：

```rust
// 旧方式
let config = ConfigLoader::new("./config").load_timeshift_config("rtsp")?;

// 新方式
let manager = ConfigManager::new(source).await?;
let config = manager.get::<TimeshiftConfig>("rtsp").await?;

// 订阅变更
manager.subscribe(|old, new| {
    println!("Config changed!");
    // 重新初始化服务
});
```

---

## ⚠️ 注意事项

### 1. 线程安全
- 使用 `Arc<RwLock<T>>` 保护共享状态
- 配置更新时加写锁
- 读取配置时加读锁

### 2. 错误处理
- 配置加载失败使用默认值
- 校验失败拒绝更新
- 保留上一个有效配置

### 3. 性能优化
- 配置缓存避免频繁读取
- 增量更新减少通知
- 异步加载不阻塞主线程

---

## 🎉 总结

配置管理系统将提供：
- ✅ 统一的配置管理接口
- ✅ 动态热更新能力
- ✅ 多数据源灵活支持
- ✅ 完善的校验和版本控制
- ✅ 生产级可靠性

**预计工期**：2-3 周  
**优先级**：高  
**复杂度**：中等

---

**下一步行动**：
1. 创建 flux-config-manager crate
2. 实现 ConfigManager 核心结构
3. 实现 FileSource
4. 编写基础测试

**规划完成时间**: 2026-02-20
