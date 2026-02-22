# flux-middleware

统一中间件包，提供认证、授权、限流和会话管理功能。

---

## 📋 功能特性

### 1. 认证（Authentication）

- ✅ JWT Token 生成和验证
- ✅ Token 刷新机制
- ✅ 过期检查
- ✅ Axum 中间件集成

### 2. 授权（Authorization）

- ✅ RBAC 权限控制
- ✅ 预定义角色（Admin/Operator/Viewer）
- ✅ 灵活的权限系统
- ✅ 用户-角色关联

### 3. 限流（Rate Limiting）

- ✅ 令牌桶算法
- ✅ 多种限流策略
  - 按 IP 限流
  - 按用户限流
  - 按资源限流
  - 带宽限流
  - 全局限流
- ✅ 自动令牌补充

### 4. 会话管理（Session Management）

- ✅ 会话存储抽象
- ✅ 内存存储（开发/测试）
- ✅ Redis 存储（生产环境）
- ✅ 自动过期清理
- ✅ 会话刷新

---

## 🚀 快速开始

### 添加依赖

```toml
[dependencies]
flux-middleware = { path = "../flux-middleware" }

# 如果需要 Redis 会话存储
flux-middleware = { path = "../flux-middleware", features = ["redis-session"] }
```

---

## 📖 使用示例

### 1. JWT 认证

```rust
use flux_middleware::JwtAuth;

#[tokio::main]
async fn main() {
    // 创建 JWT 认证管理器
    let jwt_auth = JwtAuth::new("your-secret-key".to_string(), 24);
    
    // 生成 token
    let token = jwt_auth.generate_token(
        "user123",
        vec!["admin".to_string()]
    ).unwrap();
    
    println!("Token: {}", token);
    
    // 验证 token
    let claims = jwt_auth.verify_token(&token).unwrap();
    println!("User ID: {}", claims.sub);
    println!("Roles: {:?}", claims.roles);
}
```

---

### 2. RBAC 权限控制

```rust
use flux_middleware::RbacManager;

#[tokio::main]
async fn main() {
    let rbac = RbacManager::new();
    
    // 等待默认角色初始化
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 为用户分配角色
    rbac.assign_role("user123", "admin").await.unwrap();
    
    // 检查权限
    let has_permission = rbac.check_permission(
        "user123",
        "streams",
        "delete"
    ).await.unwrap();
    
    println!("Has permission: {}", has_permission);
}
```

---

### 3. Axum 中间件集成

```rust
use axum::{Router, routing::get};
use flux_middleware::auth::{jwt_middleware, require_permission};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let jwt_auth = Arc::new(JwtAuth::default());
    let rbac = Arc::new(RbacManager::new());
    
    let app = Router::new()
        .route("/api/streams", get(list_streams))
            .layer(axum::middleware::from_fn_with_state(
                rbac.clone(),
                require_permission("streams", "read")
            ))
        .layer(axum::middleware::from_fn_with_state(
            jwt_auth,
            jwt_middleware
        ));
    
    // 启动服务器...
}

async fn list_streams() -> &'static str {
    "Stream list"
}
```

---

### 4. 限流器

```rust
use flux_middleware::{RateLimiter, RateLimitStrategy};

#[tokio::main]
async fn main() {
    let limiter = RateLimiter::new(vec![
        RateLimitStrategy::by_ip(100, 60),  // 每分钟 100 个请求
        RateLimitStrategy::global(1000, 60), // 全局每分钟 1000 个请求
    ]);
    
    // 检查是否允许请求
    if limiter.check("192.168.1.1").await {
        println!("Request allowed");
    } else {
        println!("Rate limit exceeded");
    }
}
```

---

### 5. 会话管理

```rust
use flux_middleware::session::{SessionManager, MemorySessionStore};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let store = Arc::new(MemorySessionStore::new());
    let manager = SessionManager::new(store, Duration::from_secs(3600));
    
    // 创建会话
    let session = manager.create_session("user123".to_string()).await.unwrap();
    println!("Session ID: {}", session.session_id);
    
    // 获取会话
    let loaded = manager.get_session(&session.session_id).await.unwrap();
    if let Some(s) = loaded {
        println!("User: {}", s.user_id);
    }
    
    // 刷新会话
    manager.refresh_session(&session.session_id).await.unwrap();
}
```

---

### 6. Redis 会话存储

```rust
use flux_middleware::session::{SessionManager, RedisSessionStore};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let store = Arc::new(
        RedisSessionStore::new(
            "redis://127.0.0.1:6379",
            Duration::from_secs(3600)
        ).unwrap()
    );
    
    let manager = SessionManager::new(store, Duration::from_secs(3600));
    
    // 使用方式与内存存储相同
}
```

---

## 🏗️ 架构设计

```
flux-middleware/
  ├─ auth/              # 认证授权模块
  │   ├─ jwt.rs         # JWT 实现
  │   ├─ rbac.rs        # RBAC 实现
  │   └─ middleware.rs  # Axum 中间件
  │
  ├─ ratelimit/         # 限流模块
  │   ├─ token_bucket.rs   # 令牌桶算法
  │   ├─ strategy.rs       # 限流策略
  │   └─ limiter.rs        # 限流器
  │
  └─ session/           # 会话管理模块
      ├─ data.rs        # 会话数据
      ├─ store.rs       # 存储抽象
      └─ manager.rs     # 会话管理器
```

---

## 🎯 核心概念

### JWT Claims

```rust
pub struct Claims {
    pub sub: String,           // 用户 ID
    pub roles: Vec<String>,    // 用户角色
    pub exp: i64,              // 过期时间
    pub iat: i64,              // 签发时间
    pub jti: String,           // JWT ID
}
```

### RBAC 角色

**预定义角色**：
- **Admin**: 完全访问权限（`*:*`）
- **Operator**: 管理流和设备（`streams:read/write`, `devices:read/write`）
- **Viewer**: 只读访问（`streams:read`, `devices:read`）

### 限流策略

```rust
pub enum RateLimitStrategy {
    ByIp { max_requests: u64, window: Duration },
    ByUser { max_requests: u64, window: Duration },
    ByResource { max_clients: u64 },
    ByBandwidth { max_mbps: u64 },
    Global { max_requests: u64, window: Duration },
}
```

---

## 🧪 测试

```bash
# 运行所有测试
cargo test -p flux-middleware

# 运行特定模块测试
cargo test -p flux-middleware auth::
cargo test -p flux-middleware ratelimit::
cargo test -p flux-middleware session::
```

---

## 📊 性能特点

### JWT 认证
- **验证速度**: < 1ms
- **Token 大小**: ~200-300 字节
- **并发安全**: 完全线程安全

### 限流器
- **检查延迟**: < 0.1ms
- **内存占用**: ~100 字节/桶
- **并发性能**: 支持高并发

### 会话管理
- **内存存储**: O(1) 查询
- **Redis 存储**: ~1-2ms 延迟
- **自动清理**: 每 5 分钟

---

## 🔧 配置示例

### 配置文件（config.toml）

```toml
[auth]
jwt_secret = "your-secret-key-change-in-production"
jwt_expiration_hours = 24

[ratelimit]
enabled = true

[[ratelimit.rules]]
type = "by_ip"
max_requests = 100
window_seconds = 60

[[ratelimit.rules]]
type = "global"
max_requests = 10000
window_seconds = 60

[session]
ttl_seconds = 3600
store_type = "redis"  # or "memory"
redis_url = "redis://127.0.0.1:6379"
```

---

## 🚨 安全建议

1. **JWT Secret**: 
   - 使用强随机密钥（至少 32 字节）
   - 定期轮换密钥
   - 不要硬编码在代码中

2. **限流配置**:
   - 根据实际负载调整阈值
   - 监控限流触发情况
   - 为关键 API 设置更严格的限制

3. **会话管理**:
   - 生产环境使用 Redis
   - 设置合理的 TTL
   - 定期清理过期会话

4. **HTTPS**:
   - 生产环境必须使用 HTTPS
   - Token 只通过 HTTPS 传输

---

## 📝 最佳实践

### 1. 认证流程

```rust
// 登录
async fn login(credentials: Credentials) -> Result<String> {
    // 验证用户名密码
    let user = verify_credentials(&credentials)?;
    
    // 生成 token
    let token = jwt_auth.generate_token(&user.id, user.roles)?;
    
    Ok(token)
}

// 受保护的路由
async fn protected_route(
    Extension(claims): Extension<Claims>,
) -> String {
    format!("Hello, user {}", claims.sub)
}
```

### 2. 权限检查

```rust
// 在业务逻辑中检查权限
async fn delete_stream(
    Extension(claims): Extension<Claims>,
    State(rbac): State<Arc<RbacManager>>,
    Path(stream_id): Path<String>,
) -> Result<(), StatusCode> {
    // 检查权限
    if !rbac.check_permission(&claims.sub, "streams", "delete").await? {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // 执行删除操作
    Ok(())
}
```

### 3. 限流应用

```rust
// 在中间件中应用限流
async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = get_client_ip(&req);
    
    if !limiter.check(&ip).await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    
    Ok(next.run(req).await)
}
```

---

## 🔗 相关包

- `flux-metrics` - 监控和指标收集
- `flux-logging` - 结构化日志
- `flux-config` - 配置管理

---

## 📄 许可证

MIT License

---

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

---

**版本**: v0.1.0  
**最后更新**: 2026-02-22
