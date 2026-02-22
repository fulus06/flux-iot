use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use super::{JwtAuth, RbacManager};

/// JWT 认证中间件
pub async fn jwt_middleware(
    State(jwt_auth): State<Arc<JwtAuth>>,
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
    let claims = jwt_auth
        .verify_token(token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // 检查是否过期
    if jwt_auth.is_expired(&claims) {
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
) -> impl Fn(State<Arc<RbacManager>>, Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>> + Clone {
    move |State(rbac): State<Arc<RbacManager>>, req: Request, next: Next| {
        Box::pin(async move {
            // 从 extensions 获取 claims
            let claims = req
                .extensions()
                .get::<super::Claims>()
                .ok_or(StatusCode::UNAUTHORIZED)?;

            // 检查权限
            let has_permission = rbac
                .check_permission(&claims.sub, resource, action)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if !has_permission {
                return Err(StatusCode::FORBIDDEN);
            }

            Ok(next.run(req).await)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request as HttpRequest};

    #[tokio::test]
    async fn test_jwt_middleware_no_header() {
        let jwt_auth = Arc::new(JwtAuth::default());
        let req = HttpRequest::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        // 这个测试需要完整的 middleware 设置，这里只是示例
        // 实际测试应该在集成测试中进行
    }
}
