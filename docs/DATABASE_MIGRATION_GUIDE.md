# FLUX IOT 数据库迁移指南

> 从 SQLite 迁移到 PostgreSQL + 集中式迁移管理
> 
> 日期: 2026-02-23
> 状态: ✅ 已实施

---

## 📋 迁移概览

### 迁移方案

- ✅ **方案 A**: 统一使用 PostgreSQL + 集中式迁移管理
- ✅ **选项 A**: 单数据库多 Schema
- ✅ **选项 B**: 使用 sea-orm-migration

### 架构设计

```
flux_iot (PostgreSQL 数据库)
├── public (默认 schema)
│   ├── users              # 用户认证
│   ├── app_config         # 应用配置
│   ├── app_config_audit   # 配置审计
│   ├── rules              # 规则引擎
│   └── events             # 事件总线
├── device
│   ├── devices            # 设备管理
│   └── device_metrics     # 设备指标
├── mqtt
│   ├── mqtt_clients       # MQTT 客户端
│   └── mqtt_subscriptions # MQTT 订阅
├── control
│   ├── device_commands    # 设备指令
│   └── command_responses  # 指令响应
└── rtmpd
    └── (预留)
```

---

## 🚀 快速开始

### 1. 启动 PostgreSQL

```bash
# 使用 Docker Compose (推荐)
docker-compose up -d postgres

# 等待数据库就绪
docker-compose logs -f postgres
```

### 2. 设置环境变量

```bash
# 复制环境变量模板
cp .env.example .env

# 编辑 .env 文件
export DATABASE_URL="postgresql://flux:flux@localhost/flux_iot"
```

### 3. 运行迁移

```bash
# 进入迁移目录
cd migration

# 运行所有迁移
cargo run -- up

# 查看迁移状态
cargo run -- status
```

### 4. 验证迁移

```bash
# 连接数据库
psql postgresql://flux:flux@localhost/flux_iot

# 查看所有 schema
\dn

# 查看 public schema 的表
\dt public.*

# 查看 device schema 的表
\dt device.*

# 退出
\q
```

---

## 📂 目录结构变化

### 旧结构 (SQLite 分散迁移)

```
crates/
├── flux-control/migrations/
│   └── 001_create_control_tables.sql
├── flux-device/migrations/
│   └── 001_create_devices_tables.sql
├── flux-middleware/migrations/
│   └── 001_create_users_table.sql
└── flux-mqtt/migrations/
    └── 001_create_mqtt_tables.sql
```

### 新结构 (PostgreSQL 集中迁移)

```
migration/                              ← 新建
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── main.rs
    ├── m20260223_000001_create_schemas.rs
    ├── m20260223_000002_create_users_table.rs
    ├── m20260223_000003_create_devices_tables.rs
    ├── m20260223_000004_create_mqtt_tables.rs
    ├── m20260223_000005_create_control_tables.rs
    └── m20260223_000006_create_config_tables.rs
```

---

## 🔄 SQLite vs PostgreSQL 对比

### 数据类型映射

| 功能 | SQLite | PostgreSQL |
|------|--------|-----------|
| 自增主键 | `INTEGER PRIMARY KEY AUTOINCREMENT` | `SERIAL` 或 `BIGSERIAL` |
| 时间戳 | `INTEGER` (Unix timestamp) | `TIMESTAMP WITH TIME ZONE` |
| 布尔值 | `INTEGER` (0/1) | `BOOLEAN` |
| JSON | `TEXT` | `JSONB` (原生支持) |
| 数组 | 不支持 | `TEXT[]`, `INTEGER[]` 等 |

### 语法差异

```sql
-- SQLite: 当前时间
created_at INTEGER DEFAULT (strftime('%s', 'now'))

-- PostgreSQL: 当前时间
created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP

-- SQLite: 冲突处理
INSERT ... ON CONFLICT(username) DO NOTHING

-- PostgreSQL: 相同语法
INSERT ... ON CONFLICT(username) DO NOTHING
```

---

## 📝 迁移文件说明

### 1. 创建 Schemas (m20260223_000001)

```rust
// 创建 4 个独立的 schema
CREATE SCHEMA IF NOT EXISTS rtmpd;
CREATE SCHEMA IF NOT EXISTS mqtt;
CREATE SCHEMA IF NOT EXISTS device;
CREATE SCHEMA IF NOT EXISTS control;
```

### 2. 用户表 (m20260223_000002)

- 位置: `public.users`
- 功能: 统一用户认证
- 特性: bcrypt 密码哈希, JSONB 角色

### 3. 设备表 (m20260223_000003)

- 位置: `device.devices`, `device.device_metrics`
- 功能: 设备管理和指标存储
- 特性: 外键约束, 时序数据索引

### 4. MQTT 表 (m20260223_000004)

- 位置: `mqtt.mqtt_clients`, `mqtt.mqtt_subscriptions`
- 功能: MQTT 客户端和订阅管理
- 特性: 级联删除

### 5. 控制表 (m20260223_000005)

- 位置: `control.device_commands`, `control.command_responses`
- 功能: 设备指令和响应
- 特性: JSONB 参数, 状态追踪

### 6. 配置表 (m20260223_000006)

- 位置: `public.app_config`, `public.rules`, `public.events`
- 功能: 应用配置、规则引擎、事件总线
- 特性: 审计日志, 版本控制

---

## 🔧 代码集成

### 在 flux-server 中使用

```rust
// main.rs
use migration::{Migrator, MigratorTrait};
use sea_orm::Database;

#[tokio::main]
async fn main() -> Result<()> {
    // 连接数据库
    let db_url = std::env::var("DATABASE_URL")?;
    let db = Database::connect(&db_url).await?;
    
    // 运行迁移
    Migrator::up(&db, None).await?;
    tracing::info!("Database migrations completed");
    
    // 启动应用
    // ...
}
```

### 更新 Cargo.toml

```toml
[dependencies]
migration = { path = "../migration" }
sea-orm = { version = "0.12", features = ["sqlx-postgres", "runtime-tokio-rustls", "macros"] }
# 移除: sqlx-sqlite
```

### 更新配置文件

```toml
# config.toml
[database]
url = "postgresql://flux:flux@localhost/flux_iot"
max_connections = 10
min_connections = 2
connect_timeout = 30
```

---

## 🛠️ 迁移命令

### 基本命令

```bash
# 运行所有待执行的迁移
cargo run -p migration -- up

# 回滚最后一次迁移
cargo run -p migration -- down

# 查看迁移状态
cargo run -p migration -- status

# 刷新所有迁移 (开发环境)
cargo run -p migration -- fresh

# 重置数据库 (危险！)
cargo run -p migration -- reset
```

### 生成新迁移

```bash
# 生成新的迁移文件
cd migration
cargo run -- generate add_user_avatar_column

# 会创建: src/m20260223_XXXXXX_add_user_avatar_column.rs
```

---

## 📊 性能优化

### 索引策略

```sql
-- 高频查询字段
CREATE INDEX idx_devices_status ON device.devices(status);
CREATE INDEX idx_device_commands_status ON control.device_commands(status);

-- 时序数据
CREATE INDEX idx_device_metrics_timestamp ON device.device_metrics(timestamp DESC);

-- 复合索引
CREATE INDEX idx_device_commands_device_status 
ON control.device_commands(device_id, status);
```

### 连接池配置

```rust
let db = Database::connect(
    ConnectOptions::new(db_url)
        .max_connections(10)
        .min_connections(2)
        .connect_timeout(Duration::from_secs(30))
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(3600))
).await?;
```

---

## 🔒 安全建议

### 1. 密码管理

```bash
# 生产环境使用强密码
POSTGRES_PASSWORD=$(openssl rand -base64 32)

# 不要在代码中硬编码密码
DATABASE_URL=postgresql://flux:${POSTGRES_PASSWORD}@localhost/flux_iot
```

### 2. 权限控制

```sql
-- 创建只读用户
CREATE USER flux_readonly WITH PASSWORD 'readonly_password';
GRANT CONNECT ON DATABASE flux_iot TO flux_readonly;
GRANT USAGE ON SCHEMA public, device, mqtt, control TO flux_readonly;
GRANT SELECT ON ALL TABLES IN SCHEMA public, device, mqtt, control TO flux_readonly;
```

### 3. SSL 连接

```bash
DATABASE_URL="postgresql://flux:password@localhost/flux_iot?sslmode=require"
```

---

## 🐛 故障排查

### 迁移失败

```bash
# 查看详细日志
RUST_LOG=debug cargo run -p migration -- up

# 检查数据库连接
psql $DATABASE_URL -c "SELECT version();"

# 查看迁移历史
psql $DATABASE_URL -c "SELECT * FROM seaql_migrations ORDER BY version;"
```

### 回滚迁移

```bash
# 回滚一次
cargo run -p migration -- down

# 回滚多次
cargo run -p migration -- down -n 3
```

### 数据库锁定

```sql
-- 查看活动连接
SELECT * FROM pg_stat_activity WHERE datname = 'flux_iot';

-- 终止连接
SELECT pg_terminate_backend(pid) FROM pg_stat_activity 
WHERE datname = 'flux_iot' AND pid <> pg_backend_pid();
```

---

## 📈 监控和维护

### 数据库大小

```sql
-- 查看数据库大小
SELECT pg_size_pretty(pg_database_size('flux_iot'));

-- 查看表大小
SELECT schemaname, tablename, 
       pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname IN ('public', 'device', 'mqtt', 'control')
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
```

### 性能分析

```sql
-- 慢查询
SELECT query, mean_exec_time, calls
FROM pg_stat_statements
ORDER BY mean_exec_time DESC
LIMIT 10;

-- 索引使用情况
SELECT schemaname, tablename, indexname, idx_scan
FROM pg_stat_user_indexes
ORDER BY idx_scan ASC;
```

---

## ✅ 迁移检查清单

### 迁移前

- [ ] 备份现有 SQLite 数据库
- [ ] 安装 PostgreSQL 15+
- [ ] 配置环境变量
- [ ] 测试数据库连接

### 迁移中

- [ ] 运行 `cargo run -p migration -- up`
- [ ] 验证所有表已创建
- [ ] 检查索引和约束
- [ ] 验证默认数据

### 迁移后

- [ ] 更新所有服务的数据库配置
- [ ] 测试应用功能
- [ ] 监控性能指标
- [ ] 删除旧的 SQLite 文件

---

## 📚 参考资料

- [SeaORM Migration 文档](https://www.sea-ql.org/SeaORM/docs/migration/)
- [PostgreSQL 官方文档](https://www.postgresql.org/docs/)
- [Schema 设计最佳实践](https://wiki.postgresql.org/wiki/Don%27t_Do_This)
- [PostgreSQL 性能调优](https://wiki.postgresql.org/wiki/Performance_Optimization)

---

## 🎯 下一步

1. ✅ 运行迁移创建所有表
2. ⏳ 更新各服务代码使用 PostgreSQL
3. ⏳ 数据迁移（如果有现有数据）
4. ⏳ 性能测试和优化
5. ⏳ 生产环境部署

**迁移已准备就绪！执行 `cargo run -p migration -- up` 开始迁移。**
