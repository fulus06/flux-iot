# flux-config-manager

统一的动态配置热更新系统，支持多数据源、版本管理和配置校验。

## 特性

- ✅ **多数据源支持**：File (TOML/JSON)、SQLite、PostgreSQL
- ✅ **热更新**：配置变更自动重载，无需重启服务
- ✅ **版本管理**：完整的配置历史记录和回滚功能
- ✅ **配置校验**：灵活的校验规则系统
- ✅ **变更通知**：订阅者模式，实时接收配置变更
- ✅ **类型安全**：泛型设计，支持任意可序列化类型

## 快速开始

### 基本使用

```rust
use flux_config_manager::{ConfigManager, FileSource};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
struct AppConfig {
    port: u16,
    host: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 创建文件源
    let source = Arc::new(FileSource::new("config.toml"));
    
    // 创建配置管理器
    let manager = Arc::new(ConfigManager::new(source));
    
    // 加载配置
    let config = manager.load().await?;
    println!("Port: {}", config.port);
    
    // 启动热更新
    manager.clone().start_watching().await?;
    
    Ok(())
}
```

### 配置校验

```rust
use flux_config_manager::{ConfigValidator, RangeRule};

let mut validator = ConfigValidator::new();

// 添加端口范围校验
validator.add_rule(Box::new(RangeRule::new(
    "port_range".to_string(),
    "port".to_string(),
    |c: &AppConfig| c.port as i64,
    1024,
    65535,
)));

// 校验配置
validator.validate(&config)?;
```

### 订阅配置变更

```rust
// 订阅变更
let mut rx = manager.subscribe().await;

tokio::spawn(async move {
    while let Some(change) = rx.recv().await {
        match change {
            ConfigChange::Updated { old, new } => {
                println!("Config updated!");
                // 重新初始化服务
            }
            _ => {}
        }
    }
});
```

### SQLite 数据源

```toml
[dependencies]
flux-config-manager = { version = "0.1", features = ["sqlite"] }
```

```rust
use flux_config_manager::SqliteSource;

let source = Arc::new(
    SqliteSource::new("sqlite://config.db", "my_service".to_string())
        .await?
);

let manager = Arc::new(ConfigManager::new(source));
```

### PostgreSQL 数据源

```toml
[dependencies]
flux-config-manager = { version = "0.1", features = ["postgres"] }
```

```rust
use flux_config_manager::PostgresSource;

let source = Arc::new(
    PostgresSource::new(
        "postgres://localhost/config",
        "my_service".to_string()
    ).await?
);

let manager = Arc::new(ConfigManager::new(source));
```

## 配置格式

### TOML

```toml
port = 8080
host = "localhost"
max_connections = 100
```

### JSON

```json
{
  "port": 8080,
  "host": "localhost",
  "max_connections": 100
}
```

## API 文档

### ConfigManager

- `new(source)` - 创建配置管理器
- `load()` - 加载配置
- `reload()` - 重新加载配置
- `update(config, author, comment)` - 更新配置
- `rollback(version)` - 回滚到指定版本
- `get()` - 获取当前配置
- `list_versions()` - 列出版本历史
- `subscribe()` - 订阅配置变更
- `start_watching()` - 启动配置监听

### ConfigSource

配置源接口，支持自定义实现：

- `load()` - 加载配置
- `save(config)` - 保存配置
- `watch()` - 监听配置变更

### ConfigValidator

- `new()` - 创建校验器
- `add_rule(rule)` - 添加校验规则
- `validate(config)` - 执行校验

## 测试

```bash
# 运行所有测试
cargo test -p flux-config-manager

# 运行包含数据库的测试
cargo test -p flux-config-manager --all-features
```

## 许可证

MIT
