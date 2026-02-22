use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// JWT 认证管理器
pub struct JwtAuth {
    secret: Arc<String>,
    expiration: Duration,
}

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,           // 用户 ID
    pub roles: Vec<String>,    // 用户角色
    pub exp: i64,              // 过期时间
    pub iat: i64,              // 签发时间
    pub jti: String,           // JWT ID
}

impl JwtAuth {
    /// 创建新的 JWT 认证管理器
    pub fn new(secret: String, expiration_hours: i64) -> Self {
        Self {
            secret: Arc::new(secret),
            expiration: Duration::hours(expiration_hours),
        }
    }

    /// 生成 JWT Token
    pub fn generate_token(&self, user_id: &str, roles: Vec<String>) -> Result<String> {
        let now = Utc::now();
        let exp = (now + self.expiration).timestamp();
        
        let claims = Claims {
            sub: user_id.to_string(),
            roles,
            exp,
            iat: now.timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )?;

        Ok(token)
    }

    /// 验证 JWT Token
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// 刷新 Token
    pub fn refresh_token(&self, token: &str) -> Result<String> {
        let claims = self.verify_token(token)?;
        
        // 生成新的 token，保持相同的用户和角色
        self.generate_token(&claims.sub, claims.roles)
    }

    /// 检查 Token 是否过期
    pub fn is_expired(&self, claims: &Claims) -> bool {
        let now = Utc::now().timestamp();
        claims.exp < now
    }
}

impl Default for JwtAuth {
    fn default() -> Self {
        Self::new("default-secret-change-in-production".to_string(), 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_token() {
        let auth = JwtAuth::new("test-secret".to_string(), 1);
        
        let token = auth.generate_token("user123", vec!["admin".to_string()]).unwrap();
        let claims = auth.verify_token(&token).unwrap();
        
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.roles, vec!["admin"]);
    }

    #[test]
    fn test_refresh_token() {
        let auth = JwtAuth::new("test-secret".to_string(), 1);
        
        let token = auth.generate_token("user123", vec!["admin".to_string()]).unwrap();
        let new_token = auth.refresh_token(&token).unwrap();
        
        let claims = auth.verify_token(&new_token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.roles, vec!["admin"]);
    }

    #[test]
    fn test_invalid_token() {
        let auth = JwtAuth::new("test-secret".to_string(), 1);
        
        let result = auth.verify_token("invalid-token");
        assert!(result.is_err());
    }
}
