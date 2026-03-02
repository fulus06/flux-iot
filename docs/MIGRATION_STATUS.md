# PostgreSQL 迁移状态报告

> 日期: 2026-02-23
> 状态: ⚠️ 准备就绪，等待手动执行

---

## ✅ 已完成的工作

### 1. 迁移基础设施创建完成

**目录结构**:
```
migration/
├── Cargo.toml                          ✅ 已创建
├── README.md                           ✅ 已创建
└── src/
    ├── lib.rs                          ✅ 已创建
    ├── main.rs                         ✅ 已创建（已修复 tokio）
    ├── m20260223_000001_create_schemas.rs           ✅ 已创建
    ├── m20260223_000002_create_users_table.rs       ✅ 已创建
    ├── m20260223_000003_create_devices_tables.rs    ✅ 已创建
    ├── m20260223_000004_create_mqtt_tables.rs       ✅ 已创建
    ├── m20260223_000005_create_control_tables.rs    ✅ 已创建
    └── m20260223_000006_create_config_tables.rs     ✅ 已创建
```

### 2. 配置文件更新完成

- ✅ `Cargo.toml` - 添加 migration 到 workspace
- ✅ `.env.example` - 添加 DATABASE_URL 配置
- ✅ `migration/Cargo.toml` - 修复为使用 tokio

### 3. 文档创建完成

- ✅ `migration/README.md` - 迁移使用说明
- ✅ `docs/DATABASE_MIGRATION_GUIDE.md` - 完整迁移指南
- ✅ `docs/POSTGRESQL_MIGRATION_SUMMARY.md` - 迁移总结

---

## ⚠️ 遇到的问题

### 1. sea-orm-cli 编译错误

**问题**: `sea-orm-cli` 依赖的 `regex` crate 版本冲突
```
error[E0277]: `regex::Error: std::error::Error` is not satisfied
```

**影响**: 
- ❌ 无法使用 `cargo run -- generate` 生成新迁移
- ✅ **不影响运行现有迁移** - `cargo run -- up` 仍然可用

**解决方案**:
- 方案 A: 等待 sea-orm-cli 更新
- 方案 B: 手动创建迁移文件（已有 6 个迁移文件足够使用）
- 方案 C: 使用 sqlx-cli 替代

### 2. Docker 环境问题

**问题**: 系统未安装 `docker-compose` 命令

**影响**: 无法自动启动 PostgreSQL

**解决方案**: 需要手动启动 PostgreSQL

---

## 🚀 手动执行步骤

### 方案 A: 使用 Docker (推荐)

```bash
# 1. 启动 PostgreSQL 容器
docker run -d \
  --name flux-postgres \
  -e POSTGRES_DB=flux_iot \
  -e POSTGRES_USER=flux \
  -e POSTGRES_PASSWORD=flux \
  -p 5432:5432 \
  -v flux-postgres-data:/var/lib/postgresql/data \
  postgres:15-alpine

# 2. 等待数据库就绪
sleep 5

# 3. 设置环境变量
export DATABASE_URL="postgresql://flux:flux@localhost/flux_iot"

# 4. 运行迁移
cd migration
cargo run --bin migration -- up

# 5. 验证迁移
docker exec flux-postgres psql -U flux -d flux_iot -c "\dn"
docker exec flux-postgres psql -U flux -d flux_iot -c "\dt *.*"
```

### 方案 B: 使用本地 PostgreSQL

```bash
# 1. 启动 PostgreSQL (如果已安装)
brew services start postgresql@15

# 2. 创建数据库和用户
createdb flux_iot
psql flux_iot -c "CREATE USER flux WITH PASSWORD 'flux';"
psql flux_iot -c "GRANT ALL PRIVILEGES ON DATABASE flux_iot TO flux;"

# 3. 设置环境变量
export DATABASE_URL="postgresql://flux:flux@localhost/flux_iot"

# 4. 运行迁移
cd migration
cargo run --bin migration -- up

# 5. 验证迁移
psql $DATABASE_URL -c "\dn"
psql $DATABASE_URL -c "\dt *.*"
```

### 方案 C: 使用 Docker Compose (需要先安装)

```bash
# 1. 安装 docker-compose
brew install docker-compose

# 2. 启动服务
docker-compose up -d postgres

# 3. 运行迁移
export DATABASE_URL="postgresql://flux:flux_secret_2026@localhost/flux_iot"
cd migration
cargo run --bin migration -- up
```

---

## 📊 迁移内容

### Schema 列表

1. **public** - 默认 schema
   - users (用户表)
   - app_config (应用配置)
   - app_config_audit (配置审计)
   - rules (规则引擎)
   - events (事件总线)

2. **device** - 设备管理
   - devices (设备表)
   - device_metrics (设备指标)

3. **mqtt** - MQTT 服务
   - mqtt_clients (MQTT 客户端)
   - mqtt_subscriptions (MQTT 订阅)

4. **control** - 设备控制
   - device_commands (设备指令)
   - command_responses (指令响应)

5. **rtmpd** - RTMPD 服务 (预留)

---

## ✅ 验证清单

执行迁移后，请验证：

```bash
# 1. 检查所有 schema 已创建
psql $DATABASE_URL -c "\dn"
# 应该看到: public, device, mqtt, control, rtmpd

# 2. 检查 public schema 的表
psql $DATABASE_URL -c "\dt public.*"
# 应该看到: users, app_config, app_config_audit, rules, events

# 3. 检查 device schema 的表
psql $DATABASE_URL -c "\dt device.*"
# 应该看到: devices, device_metrics

# 4. 检查 mqtt schema 的表
psql $DATABASE_URL -c "\dt mqtt.*"
# 应该看到: mqtt_clients, mqtt_subscriptions

# 5. 检查 control schema 的表
psql $DATABASE_URL -c "\dt control.*"
# 应该看到: device_commands, command_responses

# 6. 检查迁移历史
psql $DATABASE_URL -c "SELECT * FROM seaql_migrations ORDER BY version;"
# 应该看到 6 条迁移记录
```

---

## 🔧 故障排查

### 迁移失败

```bash
# 查看详细日志
RUST_LOG=debug cargo run --bin migration -- up

# 检查数据库连接
psql $DATABASE_URL -c "SELECT version();"
```

### 回滚迁移

```bash
# 回滚最后一次迁移
cargo run --bin migration -- down

# 查看迁移状态
cargo run --bin migration -- status
```

---

## 📝 下一步

1. **选择并执行上述方案之一**启动 PostgreSQL
2. **运行迁移** - `cd migration && cargo run --bin migration -- up`
3. **验证迁移成功** - 使用上述验证清单
4. **更新服务代码** - 将所有服务从 SQLite 切换到 PostgreSQL
5. **测试功能** - 确保所有功能正常工作

---

## 📚 相关文档

- `migration/README.md` - 迁移工具使用说明
- `docs/DATABASE_MIGRATION_GUIDE.md` - 完整迁移指南
- `docs/POSTGRESQL_MIGRATION_SUMMARY.md` - 迁移方案总结

---

**迁移基础设施已完成！请选择上述方案之一手动启动 PostgreSQL 并执行迁移。** 🚀
