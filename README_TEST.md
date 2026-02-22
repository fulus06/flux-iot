# FLUX IOT 测试配置说明

## 数据库配置

测试使用 **PostgreSQL** 数据库。

### 连接信息

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test
```

- **主机**: localhost
- **端口**: 5432
- **用户**: postgres
- **密码**: postgres
- **数据库**: flux_test

## 快速开始

### 1. 初始化测试数据库

使用自动化脚本：

```bash
./scripts/setup_test_db.sh
```

或手动执行：

```bash
# 创建数据库
psql -U postgres -c "CREATE DATABASE flux_test;"

# 初始化表结构
psql -U postgres -d flux_test -f scripts/init_test_db.sql
```

### 2. 运行测试

#### 使用 Makefile（推荐）

```bash
# 运行所有测试
make test

# 运行单元测试
make test-unit

# 运行集成测试
make test-integration

# 运行特定模块测试
make test-server
make test-mqtt
make test-device
make test-video
```

#### 使用 Cargo 直接运行

```bash
# 所有测试
DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test cargo test

# 单元测试
DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test cargo test --lib

# 集成测试
DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test cargo test --test '*'

# 特定模块
DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test cargo test -p flux-device
```

#### 使用环境变量文件

```bash
# 加载环境变量
source .env.test

# 运行测试
cargo test
```

## 配置文件

### 1. `.env.test`

测试环境变量配置文件：

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test
RUST_LOG=info,flux_server=debug
RUST_TEST_THREADS=4
```

### 2. `.cargo/config.toml`

Cargo 自动加载的配置：

```toml
[env]
DATABASE_URL = "postgres://postgres:postgres@localhost:5432/flux_test"
```

### 3. `Makefile`

所有测试命令已配置 DATABASE_URL：

```makefile
test-unit:
    DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
        cargo test --lib --all-features
```

## CI/CD 配置

GitHub Actions 已配置 PostgreSQL 服务和环境变量。

查看 `.github/workflows/test.yml` 了解详情。

## 数据库管理

### 重置测试数据库

```bash
# 删除并重建
psql -U postgres -c "DROP DATABASE IF EXISTS flux_test;"
psql -U postgres -c "CREATE DATABASE flux_test;"
psql -U postgres -d flux_test -f scripts/init_test_db.sql
```

### 查看表结构

```bash
psql -U postgres -d flux_test -c "\dt"
```

### 清空数据

```bash
psql -U postgres -d flux_test -c "TRUNCATE devices, device_groups, device_metrics, rules, events CASCADE;"
```

## 故障排查

### 问题 1: 数据库连接失败

```
Error: Failed to connect to PostgreSQL
```

**解决方案**:
1. 确认 PostgreSQL 服务运行: `pg_isready`
2. 检查用户名密码是否正确
3. 确认数据库已创建: `psql -U postgres -l | grep flux_test`

### 问题 2: 表不存在

```
Error: relation "devices" does not exist
```

**解决方案**:
```bash
psql -U postgres -d flux_test -f scripts/init_test_db.sql
```

### 问题 3: 类型转换错误

```
Error: column "tags" is of type text[] but expression is of type jsonb
```

**解决方案**: 查看 `DATABASE_MIGRATION_GUIDE.md` 了解详情。

## 相关文档

- `docs/TESTING_GUIDE.md` - 完整测试指南
- `DATABASE_MIGRATION_GUIDE.md` - 数据库迁移指南
- `TEST_SUMMARY.md` - 测试结果总结
- `scripts/init_test_db.sql` - 数据库初始化脚本
