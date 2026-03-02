# UserRepository 迁移到 flux-middleware

> 日期: 2026-02-23
> 状态: ✅ 已完成

## 背景

UserRepository 原本在 `flux-rtmpd` 中实现，仅供 RTMPD 使用。为了提高代码复用性和统一认证架构，将其迁移到 `flux-middleware` 包中。

## 迁移原因

1. **统一认证模块** - flux-middleware 已包含 JwtAuth 和 RbacManager，UserRepository 是自然的补充
2. **提高复用性** - 其他服务（flux-server、flux-gb28181d 等）可以直接使用
3. **清晰的职责划分** - 认证相关功能集中在一个包中
4. **避免重复实现** - 不需要每个服务都实现自己的用户管理

## 迁移内容

### 1. 新增文件（flux-middleware）

```
flux-middleware/
├── src/
│   └── user/
│       ├── entities.rs      ← 从 flux-rtmpd/src/db/entities.rs 移动
│       ├── repository.rs    ← 从 flux-rtmpd/src/db/repository.rs 移动
│       └── mod.rs           ← 新建
└── migrations/
    └── 001_create_users_table.sql  ← 从 flux-rtmpd/migrations/ 复制
```

### 2. 修改的文件

#### flux-middleware/Cargo.toml
```toml
[dependencies]
# ... 其他依赖
sea-orm = { version = "0.12", features = ["sqlx-sqlite", "runtime-tokio-rustls"], optional = true }

[features]
default = []
redis-session = ["redis"]
persistence = ["sea-orm"]  # 新增
```

#### flux-middleware/src/lib.rs
```rust
pub mod auth;
pub mod ratelimit;
pub mod session;

#[cfg(feature = "persistence")]
pub mod user;  // 新增

pub use auth::{JwtAuth, RbacManager, Claims, Role, Permission};
pub use ratelimit::{RateLimiter, RateLimitStrategy, TokenBucket};
pub use session::{SessionManager, SessionStore, SessionData};

#[cfg(feature = "persistence")]
pub use user::{User, UserRepository};  // 新增
```

#### flux-rtmpd/Cargo.toml
```toml
[dependencies]
flux-middleware = { path = "../flux-middleware", features = ["persistence"] }  # 启用 persistence
# 移除: sea-orm（不再需要，由 flux-middleware 提供）
```

#### flux-rtmpd/src/main.rs
```rust
mod auth;
// 移除: mod db;
mod hls_manager;
// ...
```

#### flux-rtmpd/src/auth.rs
```rust
#[cfg(feature = "persistence")]
use flux_middleware::UserRepository;  // 从 flux-middleware 导入
// 移除: use crate::db::UserRepository;
```

### 3. 删除的文件（flux-rtmpd）

- ❌ `crates/flux-rtmpd/src/db/` - 整个目录已删除
  - `db/entities.rs`
  - `db/repository.rs`
  - `db/mod.rs`

## 代码修改细节

### DateTime 类型修正

为了兼容 sea-orm，将 `DateTime<Utc>` 改为 `chrono::NaiveDateTime`：

```rust
// entities.rs
pub struct Model {
    // ...
    pub created_at: chrono::NaiveDateTime,  // 原: DateTime
    pub updated_at: Option<chrono::NaiveDateTime>,  // 原: Option<DateTime>
}

// repository.rs
created_at: Set(chrono::Local::now().naive_local()),  // 原: chrono::Utc::now()
updated_at: Set(Some(chrono::Local::now().naive_local())),  // 原: Some(chrono::Utc::now())
```

## 使用方式

### 在 flux-rtmpd 中使用（已更新）

```rust
// Cargo.toml
[dependencies]
flux-middleware = { path = "../flux-middleware", features = ["persistence"] }

// main.rs
use flux_middleware::{JwtAuth, RbacManager, UserRepository};

#[cfg(feature = "persistence")]
{
    let db = sea_orm::Database::connect("sqlite://rtmpd.db").await?;
    let user_repo = Arc::new(UserRepository::new(Arc::new(db)));
    
    let state = AppState {
        jwt_auth: Arc::new(JwtAuth::new(...)),
        rbac_manager: Arc::new(RbacManager::new()),
        user_repository: user_repo,  // 新增
        // ...
    };
}

// auth.rs
#[cfg(feature = "persistence")]
use flux_middleware::UserRepository;

async fn verify_credentials(
    username: &str,
    password: &str,
    repository: &UserRepository,
) -> Result<(String, Vec<String>)> {
    let user = repository.find_by_username(username).await?
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;
    
    // 验证密码
    if !bcrypt::verify(password, &user.password_hash)? {
        return Err(anyhow::anyhow!("Invalid password"));
    }
    
    Ok((user.id, user.get_roles()))
}
```

### 在其他服务中使用（未来）

```rust
// flux-server/Cargo.toml
[dependencies]
flux-middleware = { path = "../flux-middleware", features = ["persistence"] }

// flux-server/src/main.rs
use flux_middleware::{JwtAuth, RbacManager, UserRepository};

let db = sea_orm::Database::connect(&config.database_url).await?;
let user_repo = Arc::new(UserRepository::new(Arc::new(db)));

// 可以在 API 中使用用户认证
```

## 编译验证

所有相关包编译成功：

```bash
✅ cargo build -p flux-middleware --features persistence
✅ cargo build -p flux-rtmpd --features persistence
```

## 迁移后的架构

```
┌─────────────────────────────────────────────────────────┐
│                   flux-middleware                        │
│  ┌──────────────────────────────────────────────────┐  │
│  │ auth/                                             │  │
│  │  ├── jwt.rs         (JwtAuth)                    │  │
│  │  ├── rbac.rs        (RbacManager)                │  │
│  │  └── middleware.rs  (JWT 中间件)                 │  │
│  ├──────────────────────────────────────────────────┤  │
│  │ user/              ← 新增                        │  │
│  │  ├── entities.rs   (User 实体)                   │  │
│  │  └── repository.rs (UserRepository)              │  │
│  ├──────────────────────────────────────────────────┤  │
│  │ ratelimit/         (RateLimiter)                 │  │
│  │ session/           (SessionManager)              │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                          ▲
                          │ 依赖
         ┌────────────────┼────────────────┐
         │                │                │
    flux-rtmpd      flux-server      其他服务
```

## 优势

1. ✅ **统一认证** - JWT、RBAC、用户管理集中在一个包
2. ✅ **代码复用** - 所有服务共享同一套用户管理逻辑
3. ✅ **易于维护** - 用户管理的修改只需在一个地方进行
4. ✅ **清晰分层** - 职责明确，依赖关系清晰
5. ✅ **可选依赖** - 通过 `persistence` feature 控制，不需要的服务不会引入 sea-orm

## 下一步

现在 UserRepository 已经迁移到 flux-middleware，接下来需要：

1. 在 flux-rtmpd 的 `main.rs` 中初始化数据库连接
2. 创建 UserRepository 实例并添加到 AppState
3. 修改 `auth.rs` 中的 `login` 函数使用真实的数据库验证
4. 应用数据库迁移（`flux-middleware/migrations/001_create_users_table.sql`）
5. 测试登录功能

详见 `docs/ISOLATED_IMPLEMENTATIONS.md` 中的 "1.2 RTMPD UserRepository 未集成到认证流程"。
