# PostgreSQL 数据库配置指南

## 当前状态

已成功将测试数据库从 SQLite 迁移到 PostgreSQL。

### 已完成的工作

1. ✅ 创建 PostgreSQL 测试数据库 `flux_test`
2. ✅ 更新测试代码使用 PostgreSQL 连接
3. ✅ 创建数据库表结构
4. ✅ 更新通用测试辅助函数

### 数据库连接信息

```bash
# 默认连接字符串
DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test

# 或通过环境变量设置
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test
```

### 数据库表结构

已创建以下表：
- `devices` - 设备表
- `device_groups` - 设备分组表
- `device_metrics` - 设备指标表
- `rules` - 规则表
- `events` - 事件表

## 当前问题

### 类型转换问题

SeaORM 实体定义与数据库表类型不匹配：

1. **时间戳字段**: 
   - 数据库定义: `BIGINT` (Unix 毫秒时间戳)
   - SeaORM 使用: `DateTime<Utc>` 
   - 问题: SeaORM 自动转换为 `TIMESTAMP WITH TIME ZONE`

2. **标签字段**:
   - 数据库定义: `TEXT[]` (PostgreSQL 数组)
   - SeaORM 使用: `Vec<String>`
   - 问题: SeaORM 序列化为 `JSONB`

## 解决方案

### 方案 1: 修改 SeaORM 实体定义（推荐）

在 `flux-device` 的数据库转换器中添加自定义类型转换：

```rust
// crates/flux-device/src/db/converter.rs

use sea_orm::sea_query::{ArrayType, ValueType};
use sea_orm::TryGetable;

// 时间戳转换
impl From<DateTime<Utc>> for sea_orm::Value {
    fn from(dt: DateTime<Utc>) -> Self {
        sea_orm::Value::BigInt(Some(dt.timestamp_millis()))
    }
}

// 标签数组转换
#[derive(Debug, Clone)]
pub struct StringArray(pub Vec<String>);

impl From<StringArray> for sea_orm::Value {
    fn from(arr: StringArray) -> Self {
        sea_orm::Value::Array(
            ArrayType::String,
            Some(Box::new(arr.0.into_iter().map(sea_orm::Value::from).collect()))
        )
    }
}
```

### 方案 2: 修改数据库表结构

将时间戳字段改为 `TIMESTAMP WITH TIME ZONE`，标签改为 `JSONB`：

```sql
-- 重新创建表
DROP TABLE IF EXISTS device_metrics CASCADE;
DROP TABLE IF EXISTS devices CASCADE;
DROP TABLE IF EXISTS device_groups CASCADE;

CREATE TABLE device_groups (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    parent_id VARCHAR(255),
    path VARCHAR(1024),
    description TEXT,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    FOREIGN KEY (parent_id) REFERENCES device_groups(id) ON DELETE CASCADE
);

CREATE TABLE devices (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    device_type VARCHAR(50) NOT NULL,
    protocol VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'Inactive',
    product_id VARCHAR(255),
    secret VARCHAR(255),
    group_id VARCHAR(255),
    metadata JSONB,
    tags JSONB,  -- 改为 JSONB
    location JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_seen TIMESTAMP WITH TIME ZONE,
    FOREIGN KEY (group_id) REFERENCES device_groups(id) ON DELETE SET NULL
);
```

### 方案 3: 使用 SQLite 进行集成测试（临时方案）

由于 flux-device 模块的数据库层可能还未完全适配 PostgreSQL，可以暂时使用 SQLite 进行集成测试：

```rust
// 恢复使用 SQLite
pub async fn create_test_db() -> DatabaseConnection {
    Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database")
}
```

但需要解决并发问题：使用文件数据库而非内存数据库。

## 运行测试

### 使用 PostgreSQL

```bash
# 1. 确保 PostgreSQL 运行
# 2. 创建数据库
psql -U postgres -c "CREATE DATABASE flux_test;"

# 3. 初始化表结构
psql -U postgres -d flux_test -f scripts/init_test_db.sql

# 4. 运行测试
DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
  cargo test -p flux-device --test integration_test
```

### 使用 SQLite（文件数据库）

```bash
# 使用文件数据库避免并发问题
DATABASE_URL=sqlite:./test_device.db?mode=rwc \
  cargo test -p flux-device --test integration_test
```

## 下一步建议

1. **短期**: 使用 SQLite 文件数据库完成集成测试
2. **中期**: 修改 SeaORM 实体定义，添加 PostgreSQL 类型转换
3. **长期**: 使用数据库迁移工具（如 `sea-orm-migration`）管理表结构

## 已修改的文件

1. `crates/flux-device/tests/integration_test.rs` - 使用 PostgreSQL 连接
2. `tests/common/mod.rs` - 通用测试辅助函数使用 PostgreSQL
3. `.env.test` - 测试环境配置
4. `scripts/init_test_db.sql` - PostgreSQL 表结构初始化脚本

## 测试结果总结

### 单元测试
- ✅ 172/172 通过 (100%)

### 集成测试
- ✅ 53/63 通过 (84.1%)
- ❌ 10/63 失败 (flux-device - 数据库类型问题)
- ⚠️ 1 编译失败 (flux-config-manager)

### 需要修复
1. flux-device 数据库类型转换
2. flux-config-manager 缺少 trait 导入
