use super::entities::{self, Model as UserModel};
use sea_orm::{entity::prelude::*, DatabaseConnection, Set};
use std::sync::Arc;
use tracing::{debug, info};

/// 用户仓库
pub struct UserRepository {
    db: Arc<DatabaseConnection>,
}

impl UserRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 根据用户名查询用户
    pub async fn find_by_username(&self, username: &str) -> anyhow::Result<Option<UserModel>> {
        let user = entities::Entity::find()
            .filter(entities::Column::Username.eq(username))
            .one(&*self.db)
            .await?;

        Ok(user)
    }

    /// 根据 ID 查询用户
    pub async fn find_by_id(&self, user_id: &str) -> anyhow::Result<Option<UserModel>> {
        let user = entities::Entity::find_by_id(user_id.to_string())
            .one(&*self.db)
            .await?;

        Ok(user)
    }

    /// 创建用户
    pub async fn create(
        &self,
        username: String,
        password_hash: String,
        roles: Vec<String>,
    ) -> anyhow::Result<UserModel> {
        let user_id = uuid::Uuid::new_v4().to_string();
        let roles_json = serde_json::to_string(&roles)?;

        let user = entities::ActiveModel {
            id: Set(user_id),
            username: Set(username),
            password_hash: Set(password_hash),
            roles: Set(roles_json),
            enabled: Set(true),
            created_at: Set(chrono::Local::now().naive_local()),
            updated_at: Set(None),
        };

        let result = user.insert(&*self.db).await?;
        
        info!(user_id = %result.id, username = %result.username, "User created");
        
        Ok(result)
    }

    /// 更新用户密码
    pub async fn update_password(
        &self,
        user_id: &str,
        new_password_hash: String,
    ) -> anyhow::Result<()> {
        let user = entities::Entity::find_by_id(user_id.to_string())
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let mut user: entities::ActiveModel = user.into();
        user.password_hash = Set(new_password_hash);
        user.updated_at = Set(Some(chrono::Local::now().naive_local()));

        user.update(&*self.db).await?;
        
        debug!(user_id = %user_id, "User password updated");
        
        Ok(())
    }

    /// 更新用户角色
    pub async fn update_roles(
        &self,
        user_id: &str,
        roles: Vec<String>,
    ) -> anyhow::Result<()> {
        let user = entities::Entity::find_by_id(user_id.to_string())
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let mut user: entities::ActiveModel = user.into();
        user.roles = Set(serde_json::to_string(&roles)?);
        user.updated_at = Set(Some(chrono::Local::now().naive_local()));

        user.update(&*self.db).await?;
        
        debug!(user_id = %user_id, roles = ?roles, "User roles updated");
        
        Ok(())
    }

    /// 启用/禁用用户
    pub async fn set_enabled(&self, user_id: &str, enabled: bool) -> anyhow::Result<()> {
        let user = entities::Entity::find_by_id(user_id.to_string())
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let mut user: entities::ActiveModel = user.into();
        user.enabled = Set(enabled);
        user.updated_at = Set(Some(chrono::Local::now().naive_local()));

        user.update(&*self.db).await?;
        
        info!(user_id = %user_id, enabled = enabled, "User status updated");
        
        Ok(())
    }

    /// 删除用户
    pub async fn delete(&self, user_id: &str) -> anyhow::Result<()> {
        entities::Entity::delete_by_id(user_id.to_string())
            .exec(&*self.db)
            .await?;

        info!(user_id = %user_id, "User deleted");
        
        Ok(())
    }

    /// 列出所有用户
    pub async fn list_all(&self) -> anyhow::Result<Vec<UserModel>> {
        let users = entities::Entity::find()
            .all(&*self.db)
            .await?;

        Ok(users)
    }
}
