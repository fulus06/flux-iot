# FLUX IOT - 下一步行动计划

> 最后更新: 2026-02-23

## ✅ 已完成的工作

### 1. 场景引擎废弃 ✅
- 删除 `flux-control/src/scene/` 整个模块
- 统一使用规则引擎（功能更强大）
- 详见：`docs/SCENE_ENGINE_DEPRECATION.md`

### 2. UserRepository 迁移到 flux-middleware ✅
- 从 `flux-rtmpd` 迁移到 `flux-middleware`
- 所有服务现在都可以使用统一的用户管理
- 详见：`docs/USER_REPOSITORY_MIGRATION.md`

### 3. RTMPD 认证集成 ✅
- 在 `main.rs` 中初始化数据库连接
- 创建 UserRepository 并添加到 AppState
- 修改 `auth.rs` 使用真实数据库验证
- 编译成功 ✅

---

## 🎯 立即需要做的事（P0）

### 1. 应用数据库迁移 ⚠️

**当前状态**: 数据库连接已初始化，但表还未创建

**需要做的**:
```bash
# 方法 A: 手动执行 SQL
sqlite3 ./data/rtmpd_users.db < crates/flux-middleware/migrations/001_create_users_table.sql

# 方法 B: 在代码中自动执行（推荐）
# 在 flux-rtmpd/src/main.rs 的数据库初始化后添加：
```

```rust
// 应用数据库迁移
let migration_sql = include_str!("../../flux-middleware/migrations/001_create_users_table.sql");
for statement in migration_sql.split(';') {
    let statement = statement.trim();
    if !statement.is_empty() {
        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            statement
        )).await?;
    }
}
tracing::info!(target: "rtmpd", "Database migrations applied");
```

### 2. 创建初始用户 ⚠️

**使用示例工具创建用户**:
```bash
cd crates/flux-rtmpd
cargo run --example create_user --features persistence

# 或者直接插入 SQL
sqlite3 ./data/rtmpd_users.db
INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at)
VALUES (
    'admin-001',
    'admin',
    '$2b$12$...',  -- 使用 bcrypt 生成
    '["admin"]',
    1,
    datetime('now')
);
```

### 3. 测试登录功能 ⚠️

**启动服务**:
```bash
# 启用 persistence feature
cargo run -p flux-rtmpd --features persistence -- --http-bind 0.0.0.0:8082

# 测试登录
curl -X POST http://localhost:8082/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}'

# 应该返回 JWT token
```

---

## 📋 剩余的孤立实现（P1）

### 1. 应用完整的数据库迁移

**位置**: 
- `flux-control/migrations/001_create_control_tables.sql`
- `flux-device/migrations/001_create_devices_tables.sql`
- `flux-mqtt/migrations/001_create_mqtt_tables.sql`

**需要做的**:
在 `flux-server/src/main.rs` 中添加迁移执行逻辑：

```rust
async fn apply_migrations(db: &DatabaseConnection) -> Result<()> {
    let backend = db.get_database_backend();
    
    let migrations = vec![
        include_str!("../../flux-control/migrations/001_create_control_tables.sql"),
        include_str!("../../flux-device/migrations/001_create_devices_tables.sql"),
        include_str!("../../flux-mqtt/migrations/001_create_mqtt_tables.sql"),
    ];
    
    for migration_sql in migrations {
        for statement in migration_sql.split(';') {
            let statement = statement.trim();
            if !statement.is_empty() {
                db.execute(Statement::from_string(backend, statement)).await?;
            }
        }
    }
    
    Ok(())
}

// 在 main 函数中调用
apply_migrations(&db).await?;
```

---

## 🔧 功能完善（P2）

### 1. 实现批量指令取消逻辑
**位置**: `flux-control/src/batch/executor.rs:156-161`
**工作量**: 小（1 小时）

### 2. 集成场景通知系统
**位置**: `flux-control/src/scene/engine.rs:195-199`
**工作量**: 小（1 小时）
**注意**: 场景引擎已废弃，此项可能不需要

### 3. 实现 CoAP Observe 取消请求
**位置**: `flux-coap/src/client.rs:196-202`
**工作量**: 小（1 小时）

### 4. 优化设备在线数量查询
**位置**: `flux-device/src/monitor.rs:275-281`
**工作量**: 小（30 分钟）

### 5. 实现插件热更新监控
**位置**: `flux-server/src/plugin_loader.rs:139-147`
**工作量**: 中等（2-3 小时）

---

## 📊 当前架构状态

### 认证架构（已完成）✅

```
flux-middleware (统一认证模块)
├── auth/
│   ├── jwt.rs         (JwtAuth) ✅
│   ├── rbac.rs        (RbacManager) ✅
│   └── middleware.rs  (JWT 中间件) ✅
├── user/              ✅
│   ├── entities.rs    (User 实体)
│   └── repository.rs  (UserRepository)
├── ratelimit/         (RateLimiter) ✅
└── session/           (SessionManager) ✅

使用服务:
- flux-rtmpd ✅ (已集成)
- flux-server (可以集成)
- 其他服务 (可以集成)
```

### 自动化架构（已完成）✅

```
flux-rule (规则引擎)
├── RuleEngine ✅
├── TriggerManager ✅
├── 内置函数 ✅
└── RuleServices ✅

已废弃:
- flux-control/scene (场景引擎) ❌ 已删除
```

---

## 🎯 优先级总结

**立即执行（今天）**:
1. ✅ 应用 RTMPD 数据库迁移
2. ✅ 创建初始用户
3. ✅ 测试登录功能

**本周内**:
4. 应用 flux-server 数据库迁移
5. 完善批量指令取消
6. 实现 CoAP Observe 取消

**本月内**:
7. 优化设备查询性能
8. 实现插件热更新

---

## 📚 相关文档

- `docs/ISOLATED_IMPLEMENTATIONS.md` - 孤立实现和未完成功能清单
- `docs/SCENE_ENGINE_DEPRECATION.md` - 场景引擎废弃说明
- `docs/USER_REPOSITORY_MIGRATION.md` - UserRepository 迁移说明
- `docs/UNIMPLEMENTED_FEATURES_CODE.md` - 未实现功能代码清单

---

## ✨ 今日成就

1. ✅ 废弃场景引擎，统一使用规则引擎
2. ✅ UserRepository 迁移到 flux-middleware
3. ✅ RTMPD 认证集成完成
4. ✅ 所有包编译成功

**下一步**: 应用数据库迁移并测试登录功能！
