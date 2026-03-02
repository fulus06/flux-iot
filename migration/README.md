# FLUX IOT 数据库迁移

本目录包含使用 `sea-orm-migration` 管理的所有数据库迁移文件。

## 架构设计

### 数据库: `flux_iot`

使用 PostgreSQL 多 Schema 架构：

```
flux_iot (数据库)
├── public (默认 schema)
│   ├── users              # 用户表
│   ├── app_config         # 应用配置
│   ├── app_config_audit   # 配置审计
│   ├── rules              # 规则
│   └── events             # 事件
├── device
│   ├── devices            # 设备表
│   └── device_metrics     # 设备指标
├── mqtt
│   ├── mqtt_clients       # MQTT 客户端
│   └── mqtt_subscriptions # MQTT 订阅
├── control
│   ├── device_commands    # 设备指令
│   └── command_responses  # 指令响应
└── rtmpd
    └── (预留用于 RTMPD 特定表)
```

## 迁移文件

| 文件 | 描述 |
|------|------|
| `m20260223_000001_create_schemas.rs` | 创建所有 Schema |
| `m20260223_000002_create_users_table.rs` | 创建用户表 (public) |
| `m20260223_000003_create_devices_tables.rs` | 创建设备相关表 (device) |
| `m20260223_000004_create_mqtt_tables.rs` | 创建 MQTT 相关表 (mqtt) |
| `m20260223_000005_create_control_tables.rs` | 创建控制相关表 (control) |
| `m20260223_000006_create_config_tables.rs` | 创建配置相关表 (public) |

## 使用方法

### 1. 启动 PostgreSQL

```bash
# 使用 Docker Compose
docker-compose up -d postgres

# 或手动启动
createdb flux_iot
psql flux_iot -c "CREATE USER flux WITH PASSWORD 'flux';"
psql flux_iot -c "GRANT ALL PRIVILEGES ON DATABASE flux_iot TO flux;"
```

### 2. 设置环境变量

```bash
export DATABASE_URL="postgresql://flux:flux@localhost/flux_iot"
```

### 3. 运行迁移

```bash
# 进入 migration 目录
cd migration

# 运行所有迁移
cargo run -- up

# 查看迁移状态
cargo run -- status

# 回滚最后一次迁移
cargo run -- down

# 刷新所有迁移（开发环境）
cargo run -- fresh

# 重置数据库（危险！）
cargo run -- reset
```

### 4. 生成新迁移

```bash
# 生成新的迁移文件
cargo run -- generate MIGRATION_NAME
```

## 与 SQLite 的区别

### 数据类型映射

| SQLite | PostgreSQL |
|--------|-----------|
| `INTEGER PRIMARY KEY AUTOINCREMENT` | `SERIAL PRIMARY KEY` 或 `BIGSERIAL PRIMARY KEY` |
| `INTEGER` (时间戳) | `TIMESTAMP WITH TIME ZONE` |
| `INTEGER` (布尔值) | `BOOLEAN` |
| `TEXT` (JSON) | `JSONB` |
| `TEXT` | `TEXT` 或 `VARCHAR(n)` |

### 语法差异

```sql
-- SQLite
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at INTEGER DEFAULT (strftime('%s', 'now'))
);

-- PostgreSQL
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
```

## 集成到应用

### 在 flux-server 中使用

```rust
use migration::{Migrator, MigratorTrait};
use sea_orm::Database;

#[tokio::main]
async fn main() -> Result<()> {
    // 连接数据库
    let db = Database::connect(&std::env::var("DATABASE_URL")?).await?;
    
    // 运行迁移
    Migrator::up(&db, None).await?;
    
    // 启动应用
    // ...
}
```

### 在 Cargo.toml 中添加依赖

```toml
[dependencies]
migration = { path = "../migration" }
```

## 开发环境配置

### .env 文件

```bash
DATABASE_URL=postgresql://flux:flux@localhost/flux_iot
RUST_LOG=debug,sea_orm_migration=info
```

### 配置文件

```toml
[database]
url = "postgresql://flux:flux@localhost/flux_iot"
max_connections = 10
min_connections = 2
connect_timeout = 30
```

## 生产环境注意事项

1. **备份数据库** - 运行迁移前务必备份
2. **测试迁移** - 在测试环境先验证
3. **监控性能** - 大表迁移可能耗时较长
4. **权限管理** - 确保数据库用户有足够权限
5. **连接池配置** - 根据负载调整连接数

## 故障排查

### 迁移失败

```bash
# 查看详细日志
RUST_LOG=debug cargo run -- up

# 检查数据库连接
psql $DATABASE_URL -c "SELECT version();"

# 查看迁移表
psql $DATABASE_URL -c "SELECT * FROM seaql_migrations;"
```

### 回滚迁移

```bash
# 回滚一次
cargo run -- down

# 回滚到指定版本
cargo run -- down -n 2
```

## 参考资料

- [SeaORM Migration 文档](https://www.sea-ql.org/SeaORM/docs/migration/setting-up-migration/)
- [PostgreSQL 文档](https://www.postgresql.org/docs/)
- [Schema 设计最佳实践](https://wiki.postgresql.org/wiki/Don%27t_Do_This)
