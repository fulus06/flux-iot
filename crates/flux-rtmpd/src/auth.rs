use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::AppState;

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 登录响应
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub roles: Vec<String>,
}

/// 登录处理
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // TODO: 实际项目中应该从数据库验证用户名密码
    // 这里简化处理，仅作演示
    
    // 验证用户名密码（示例）
    let (user_id, roles) = match verify_credentials(&req.username, &req.password).await {
        Ok(user) => user,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };
    
    // 为用户分配角色（如果还没有）
    for role in &roles {
        let _ = state.rbac_manager.assign_role(&user_id, role).await;
    }
    
    // 生成 JWT token
    let token = state
        .jwt_auth
        .generate_token(&user_id, roles.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    info!(
        target: "auth",
        user_id = %user_id,
        roles = ?roles,
        "User logged in successfully"
    );
    
    Ok(Json(LoginResponse {
        token,
        user_id,
        roles,
    }))
}

/// 验证用户凭据（示例实现）
async fn verify_credentials(
    username: &str,
    password: &str,
) -> Result<(String, Vec<String>), anyhow::Error> {
    // TODO: 实际项目中应该：
    // 1. 从数据库查询用户
    // 2. 验证密码哈希（使用 bcrypt）
    // 3. 返回用户信息和角色
    
    // 示例：硬编码的测试用户
    match (username, password) {
        ("admin", "admin123") => {
            Ok(("admin".to_string(), vec!["admin".to_string()]))
        }
        ("operator", "op123") => {
            Ok(("operator".to_string(), vec!["operator".to_string()]))
        }
        ("viewer", "view123") => {
            Ok(("viewer".to_string(), vec!["viewer".to_string()]))
        }
        _ => Err(anyhow::anyhow!("Invalid credentials")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_verify_credentials() {
        let result = verify_credentials("admin", "admin123").await;
        assert!(result.is_ok());
        
        let (user_id, roles) = result.unwrap();
        assert_eq!(user_id, "admin");
        assert_eq!(roles, vec!["admin"]);
    }

    #[tokio::test]
    async fn test_invalid_credentials() {
        let result = verify_credentials("invalid", "wrong").await;
        assert!(result.is_err());
    }
}
