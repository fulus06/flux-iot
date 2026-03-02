use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 用户实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rtmp_users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    
    /// 用户名（唯一）
    #[sea_orm(unique)]
    pub username: String,
    
    /// 密码哈希（bcrypt）
    pub password_hash: String,
    
    /// 角色列表（JSON 数组）
    pub roles: String,
    
    /// 是否启用
    pub enabled: bool,
    
    /// 创建时间
    pub created_at: chrono::NaiveDateTime,
    
    /// 更新时间
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 获取角色列表
    pub fn get_roles(&self) -> Vec<String> {
        serde_json::from_str(&self.roles).unwrap_or_default()
    }
    
    /// 设置角色列表
    pub fn set_roles(&mut self, roles: Vec<String>) {
        self.roles = serde_json::to_string(&roles).unwrap_or_else(|_| "[]".to_string());
    }
}
