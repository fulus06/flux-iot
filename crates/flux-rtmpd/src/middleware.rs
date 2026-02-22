use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::warn;

use crate::AppState;

/// 限流中间件
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 获取客户端 IP
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            req.extensions()
                .get::<std::net::SocketAddr>()
                .map(|addr| addr.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    // 检查限流
    if !state.rate_limiter.check(&ip).await {
        warn!(target: "rate_limit", ip = %ip, "Rate limit exceeded");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    Ok(next.run(req).await)
}

/// JWT 认证中间件
pub async fn jwt_auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 从 Authorization header 提取 token
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 检查 Bearer 前缀
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 验证 token
    let claims = state
        .jwt_auth
        .verify_token(token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // 检查是否过期
    if state.jwt_auth.is_expired(&claims) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 将 claims 注入到 request extensions
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// 权限检查中间件生成器
pub fn require_permission(
    resource: &'static str,
    action: &'static str,
) -> impl Fn(State<AppState>, Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>> + Clone {
    move |State(state): State<AppState>, req: Request, next: Next| {
        Box::pin(async move {
            // 从 extensions 获取 claims
            let claims = req
                .extensions()
                .get::<flux_middleware::Claims>()
                .ok_or(StatusCode::UNAUTHORIZED)?;

            // 检查权限
            let has_permission = state
                .rbac_manager
                .check_permission(&claims.sub, resource, action)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if !has_permission {
                warn!(
                    target: "rbac",
                    user_id = %claims.sub,
                    resource = resource,
                    action = action,
                    "Permission denied"
                );
                return Err(StatusCode::FORBIDDEN);
            }

            Ok(next.run(req).await)
        })
    }
}
