use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// RBAC 权限管理器
pub struct RbacManager {
    roles: Arc<RwLock<HashMap<String, Role>>>,
    user_roles: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

/// 角色定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub description: String,
    pub permissions: Vec<Permission>,
}

/// 权限定义
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    pub resource: String,  // 资源类型：streams, devices, rules
    pub action: String,    // 操作：read, write, delete, execute
}

impl RbacManager {
    /// 创建新的 RBAC 管理器
    pub fn new() -> Self {
        let manager = Self {
            roles: Arc::new(RwLock::new(HashMap::new())),
            user_roles: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // 同步初始化默认角色
        let roles = manager.roles.clone();
        tokio::spawn(async move {
            Self::init_default_roles_internal(roles).await;
        });
        
        manager
    }

    /// 初始化默认角色（内部方法）
    async fn init_default_roles_internal(roles: Arc<RwLock<HashMap<String, Role>>>) {
        // Admin 角色
        let admin = Role {
            id: "admin".to_string(),
            name: "Administrator".to_string(),
            description: "Full system access".to_string(),
            permissions: vec![
                Permission { resource: "*".to_string(), action: "*".to_string() },
            ],
        };

        // Operator 角色
        let operator = Role {
            id: "operator".to_string(),
            name: "Operator".to_string(),
            description: "Can manage streams and devices".to_string(),
            permissions: vec![
                Permission { resource: "streams".to_string(), action: "read".to_string() },
                Permission { resource: "streams".to_string(), action: "write".to_string() },
                Permission { resource: "devices".to_string(), action: "read".to_string() },
                Permission { resource: "devices".to_string(), action: "write".to_string() },
            ],
        };

        // Viewer 角色
        let viewer = Role {
            id: "viewer".to_string(),
            name: "Viewer".to_string(),
            description: "Read-only access".to_string(),
            permissions: vec![
                Permission { resource: "streams".to_string(), action: "read".to_string() },
                Permission { resource: "devices".to_string(), action: "read".to_string() },
            ],
        };

        let mut role_map = roles.write().await;
        role_map.insert("admin".to_string(), admin);
        role_map.insert("operator".to_string(), operator);
        role_map.insert("viewer".to_string(), viewer);
    }

    /// 添加角色
    pub async fn add_role(&self, role: Role) -> Result<()> {
        let mut roles = self.roles.write().await;
        roles.insert(role.id.clone(), role);
        Ok(())
    }

    /// 获取角色
    pub async fn get_role(&self, role_id: &str) -> Option<Role> {
        let roles = self.roles.read().await;
        roles.get(role_id).cloned()
    }

    /// 为用户分配角色
    pub async fn assign_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        let mut user_roles = self.user_roles.write().await;
        user_roles
            .entry(user_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(role_id.to_string());
        Ok(())
    }

    /// 移除用户角色
    pub async fn remove_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        let mut user_roles = self.user_roles.write().await;
        if let Some(roles) = user_roles.get_mut(user_id) {
            roles.remove(role_id);
        }
        Ok(())
    }

    /// 获取用户的所有角色
    pub async fn get_user_roles(&self, user_id: &str) -> Vec<Role> {
        let user_roles = self.user_roles.read().await;
        let roles = self.roles.read().await;
        
        let role_ids = user_roles.get(user_id);
        if role_ids.is_none() {
            return Vec::new();
        }

        role_ids
            .unwrap()
            .iter()
            .filter_map(|role_id| roles.get(role_id).cloned())
            .collect()
    }

    /// 检查用户是否有权限
    pub async fn check_permission(
        &self,
        user_id: &str,
        resource: &str,
        action: &str,
    ) -> Result<bool> {
        let user_roles = self.get_user_roles(user_id).await;
        
        for role in user_roles {
            for perm in role.permissions {
                // 检查通配符权限
                if perm.resource == "*" && perm.action == "*" {
                    return Ok(true);
                }
                
                // 检查资源匹配
                if perm.resource == "*" || perm.resource == resource {
                    // 检查操作匹配
                    if perm.action == "*" || perm.action == action {
                        return Ok(true);
                    }
                }
            }
        }
        
        Ok(false)
    }
}

impl Default for RbacManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_assign_and_check_permission() {
        let manager = RbacManager::new();
        
        // 等待默认角色初始化
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // 分配 admin 角色
        manager.assign_role("user1", "admin").await.unwrap();
        
        // 检查权限
        let has_perm = manager
            .check_permission("user1", "streams", "delete")
            .await
            .unwrap();
        
        assert!(has_perm);
    }

    #[tokio::test]
    async fn test_viewer_permissions() {
        let manager = RbacManager::new();
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        manager.assign_role("user2", "viewer").await.unwrap();
        
        // Viewer 应该有读权限
        let can_read = manager
            .check_permission("user2", "streams", "read")
            .await
            .unwrap();
        assert!(can_read);
        
        // Viewer 不应该有写权限
        let can_write = manager
            .check_permission("user2", "streams", "write")
            .await
            .unwrap();
        assert!(!can_write);
    }
}
