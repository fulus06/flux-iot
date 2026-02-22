# flux-rtmpd 安全功能使用指南

> **版本**: v1.0  
> **日期**: 2026-02-22  
> **状态**: 已集成 flux-middleware

---

## 📋 功能概述

flux-rtmpd 已集成完整的安全功能：

- ✅ **JWT 认证** - 保护所有 HTTP API
- ✅ **RBAC 权限控制** - 基于角色的访问控制
- ✅ **限流保护** - 防止滥用和过载
- ✅ **会话管理** - 用户会话追踪

---

## 🚀 快速开始

### 1. 环境变量配置

```bash
# JWT 密钥（生产环境必须修改）
export JWT_SECRET="your-super-secret-key-change-in-production"

# 启动服务
cargo run -p flux-rtmpd
```

---

## 🔐 认证流程

### 步骤 1：登录获取 Token

```bash
curl -X POST http://localhost:3000/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "admin123"
  }'
```

**响应**：
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user_id": "admin",
  "roles": ["admin"]
}
```

### 步骤 2：使用 Token 访问受保护的 API

```bash
curl http://localhost:3000/api/v1/rtmp/streams \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

---

## 👥 预定义用户

### 测试用户（仅用于开发）

| 用户名 | 密码 | 角色 | 权限 |
|--------|------|------|------|
| `admin` | `admin123` | Admin | 完全访问权限 |
| `operator` | `op123` | Operator | 管理流和设备 |
| `viewer` | `view123` | Viewer | 只读访问 |

⚠️ **生产环境警告**：请修改 `src/auth.rs` 中的 `verify_credentials` 函数，连接到真实的数据库。

---

## 🛡️ API 路由保护

### 公开路由（无需认证）

```
GET  /health              # 健康检查
POST /login               # 登录接口
```

### 受保护的 API 路由（需要认证 + 权限）

```
GET  /api/v1/rtmp/streams              # 需要 "streams:read" 权限
GET  /api/v1/rtmp/streams/:id/snapshot # 需要 "streams:read" 权限
```

### 流媒体路由（限流保护）

```
GET  /hls/:stream_id/index.m3u8   # 限流：100次/分钟/IP
GET  /hls/:stream_id/:segment     # 限流：100次/分钟/IP
GET  /flv/:app/:stream.flv        # 限流：100次/分钟/IP
```

---

## 🎯 权限系统

### 角色定义

#### Admin 角色
- **权限**: `*:*`（所有资源的所有操作）
- **用途**: 系统管理员

#### Operator 角色
- **权限**:
  - `streams:read`
  - `streams:write`
  - `devices:read`
  - `devices:write`
- **用途**: 运维人员

#### Viewer 角色
- **权限**:
  - `streams:read`
  - `devices:read`
- **用途**: 只读用户

### 权限检查示例

```bash
# Admin 可以访问所有 API
curl -H "Authorization: Bearer <admin_token>" \
  http://localhost:3000/api/v1/rtmp/streams

# Viewer 只能读取，不能删除
curl -X DELETE -H "Authorization: Bearer <viewer_token>" \
  http://localhost:3000/api/v1/rtmp/streams/test
# 返回: 403 Forbidden
```

---

## 🚦 限流配置

### 当前限流策略

1. **按 IP 限流**: 每分钟 100 个请求
2. **全局限流**: 每分钟 10,000 个请求
3. **资源限流**: 每个流最多 1,000 个客户端

### 限流响应

当触发限流时，返回：
```
HTTP/1.1 429 Too Many Requests
```

### 修改限流配置

编辑 `src/main.rs`：

```rust
let rate_limiter = Arc::new(flux_middleware::RateLimiter::new(vec![
    flux_middleware::RateLimitStrategy::by_ip(200, 60),      // 改为 200次/分钟
    flux_middleware::RateLimitStrategy::global(20000, 60),   // 改为 20000次/分钟
    flux_middleware::RateLimitStrategy::by_resource(2000),   // 改为 2000个客户端
]));
```

---

## 🔧 生产环境部署

### 1. 修改 JWT 密钥

```bash
# 生成强随机密钥
openssl rand -base64 32

# 设置环境变量
export JWT_SECRET="生成的随机密钥"
```

### 2. 实现真实的用户验证

修改 `src/auth.rs` 中的 `verify_credentials` 函数：

```rust
async fn verify_credentials(
    username: &str,
    password: &str,
) -> Result<(String, Vec<String>), anyhow::Error> {
    // 1. 从数据库查询用户
    let user = db.query_user(username).await?;
    
    // 2. 验证密码哈希
    if !bcrypt::verify(password, &user.password_hash)? {
        return Err(anyhow::anyhow!("Invalid password"));
    }
    
    // 3. 返回用户信息和角色
    Ok((user.id, user.roles))
}
```

### 3. 配置 HTTPS

```rust
// 使用 rustls 配置 HTTPS
let tls_config = RustlsConfig::from_pem_file(
    "/path/to/cert.pem",
    "/path/to/key.pem"
).await?;

axum_server::bind_rustls(addr, tls_config)
    .serve(app.into_make_service())
    .await?;
```

---

## 📊 监控和日志

### 认证日志

```
2026-02-22T15:30:00Z INFO auth user_id=admin roles=["admin"] User logged in successfully
```

### 限流日志

```
2026-02-22T15:30:05Z WARN rate_limit ip=192.168.1.100 Rate limit exceeded
```

### 权限拒绝日志

```
2026-02-22T15:30:10Z WARN rbac user_id=viewer resource=streams action=delete Permission denied
```

---

## 🧪 测试

### 测试登录

```bash
# 成功登录
curl -X POST http://localhost:3000/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}'

# 失败登录
curl -X POST http://localhost:3000/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "wrong"}'
# 返回: 401 Unauthorized
```

### 测试权限

```bash
# 获取 token
TOKEN=$(curl -s -X POST http://localhost:3000/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}' \
  | jq -r '.token')

# 使用 token 访问 API
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/v1/rtmp/streams
```

### 测试限流

```bash
# 快速发送多个请求
for i in {1..150}; do
  curl http://localhost:3000/flv/live/test.flv &
done

# 第 101 个请求开始会返回 429
```

---

## 🔒 安全最佳实践

### 1. Token 管理
- ✅ Token 有效期设置为 24 小时
- ✅ 使用 HTTPS 传输 Token
- ✅ 不要在 URL 中传递 Token
- ✅ 定期轮换 JWT 密钥

### 2. 密码策略
- ✅ 使用 bcrypt 哈希密码
- ✅ 强制密码复杂度
- ✅ 实施密码过期策略
- ✅ 记录登录失败次数

### 3. 限流策略
- ✅ 根据实际负载调整阈值
- ✅ 为关键 API 设置更严格限制
- ✅ 监控限流触发情况
- ✅ 实施渐进式限流

### 4. 审计日志
- ✅ 记录所有认证事件
- ✅ 记录权限拒绝事件
- ✅ 记录敏感操作
- ✅ 定期审查日志

---

## 🐛 故障排查

### 问题 1: 401 Unauthorized

**原因**：Token 无效或过期

**解决**：
1. 检查 Token 是否正确
2. 检查 Token 是否过期
3. 重新登录获取新 Token

### 问题 2: 403 Forbidden

**原因**：用户没有权限

**解决**：
1. 检查用户角色
2. 检查所需权限
3. 联系管理员分配权限

### 问题 3: 429 Too Many Requests

**原因**：触发限流

**解决**：
1. 降低请求频率
2. 联系管理员调整限流配置
3. 使用缓存减少请求

---

## 📚 相关文档

- [flux-middleware README](../flux-middleware/README.md) - 中间件详细文档
- [JWT 规范](https://jwt.io/) - JWT Token 标准
- [RBAC 模型](https://en.wikipedia.org/wiki/Role-based_access_control) - 基于角色的访问控制

---

## 🔄 更新日志

### v1.0 (2026-02-22)
- ✅ 集成 flux-middleware
- ✅ 实现 JWT 认证
- ✅ 实现 RBAC 权限控制
- ✅ 实现限流保护
- ✅ 添加登录接口

---

**维护者**: FLUX IOT Team  
**最后更新**: 2026-02-22
