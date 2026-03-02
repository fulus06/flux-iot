use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::AppState;

#[cfg(feature = "persistence")]
use flux_middleware::UserRepository;

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
    // 验证用户名密码
    #[cfg(feature = "persistence")]
    let (user_id, roles) = {
        // 使用数据库验证
        match verify_credentials(&req.username, &req.password, &state.user_repository).await {
            Ok(user) => user,
            Err(e) => {
                tracing::warn!(username = %req.username, error = %e, "Login failed");
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    };

    #[cfg(not(feature = "persistence"))]
    let (user_id, roles) = match verify_credentials_fallback(&req.username, &req.password).await {
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

/// 验证用户凭据（数据库 + bcrypt）
#[cfg(feature = "persistence")]
async fn verify_credentials(
    username: &str,
    password: &str,
    repository: &UserRepository,
) -> Result<(String, Vec<String>), anyhow::Error> {
    // 1. 从数据库查询用户
    let user = repository
        .find_by_username(username)
        .await?
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;
    
    // 2. 检查用户是否启用
    if !user.enabled {
        warn!(username = %username, "Login attempt for disabled user");
        return Err(anyhow::anyhow!("User is disabled"));
    }
    
    // 3. 验证密码哈希（使用 bcrypt）
    let password_valid = bcrypt::verify(password, &user.password_hash)
        .map_err(|e| anyhow::anyhow!("Password verification failed: {}", e))?;
    
    if !password_valid {
        warn!(username = %username, "Invalid password attempt");
        return Err(anyhow::anyhow!("Invalid password"));
    }
    
    // 4. 返回用户信息和角色
    let roles = user.get_roles();
    
    Ok((user.id, roles))
}

/// 验证用户凭据（回退到示例实现）
#[cfg(not(feature = "persistence"))]
async fn verify_credentials(
    username: &str,
    password: &str,
) -> Result<(String, Vec<String>), anyhow::Error> {
    verify_credentials_fallback(username, password).await
}

/// 示例凭据验证（用于测试和回退）
async fn verify_credentials_fallback(
    username: &str,
    password: &str,
) -> Result<(String, Vec<String>), anyhow::Error> {
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
