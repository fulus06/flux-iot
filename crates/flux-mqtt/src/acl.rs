use crate::topic_matcher::TopicMatcher;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// ACL 动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AclAction {
    Publish,
    Subscribe,
    Both,
}

/// ACL 权限
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AclPermission {
    Allow,
    Deny,
}

/// ACL 规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    pub client_id: Option<String>,
    pub username: Option<String>,
    pub topic_pattern: String,
    pub action: AclAction,
    pub permission: AclPermission,
    pub priority: i32,
}

/// ACL 访问控制
#[derive(Clone)]
pub struct MqttAcl {
    rules: Arc<Vec<AclRule>>,
}

impl MqttAcl {
    pub fn new(rules: Vec<AclRule>) -> Self {
        // 按优先级排序（高优先级在前）
        let mut sorted_rules = rules;
        sorted_rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        Self {
            rules: Arc::new(sorted_rules),
        }
    }

    /// 检查发布权限
    pub fn check_publish(&self, client_id: &str, username: Option<&str>, topic: &str) -> bool {
        self.check_permission(client_id, username, topic, AclAction::Publish)
    }

    /// 检查订阅权限
    pub fn check_subscribe(&self, client_id: &str, username: Option<&str>, topic: &str) -> bool {
        self.check_permission(client_id, username, topic, AclAction::Subscribe)
    }

    /// 检查权限
    fn check_permission(
        &self,
        client_id: &str,
        username: Option<&str>,
        topic: &str,
        action: AclAction,
    ) -> bool {
        for rule in self.rules.iter() {
            // 检查规则是否匹配
            if !self.matches_rule(rule, client_id, username) {
                continue;
            }

            // 检查动作是否匹配
            if !self.matches_action(rule.action, action) {
                continue;
            }

            // 检查主题是否匹配
            if !TopicMatcher::matches(&rule.topic_pattern, topic) {
                continue;
            }

            // 找到匹配的规则，返回权限
            let allowed = matches!(rule.permission, AclPermission::Allow);
            debug!(
                client_id = %client_id,
                username = ?username,
                topic = %topic,
                action = ?action,
                permission = ?rule.permission,
                "ACL rule matched"
            );
            return allowed;
        }

        // 默认拒绝
        debug!(
            client_id = %client_id,
            username = ?username,
            topic = %topic,
            action = ?action,
            "No ACL rule matched, denying by default"
        );
        false
    }

    /// 检查规则是否匹配客户端
    fn matches_rule(&self, rule: &AclRule, client_id: &str, username: Option<&str>) -> bool {
        // 如果规则指定了 client_id，必须匹配
        if let Some(rule_client_id) = &rule.client_id {
            if !self.matches_pattern(rule_client_id, client_id) {
                return false;
            }
        }

        // 如果规则指定了 username，必须匹配
        if let Some(rule_username) = &rule.username {
            match username {
                Some(user) => {
                    if !self.matches_pattern(rule_username, user) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }

    /// 检查动作是否匹配
    fn matches_action(&self, rule_action: AclAction, requested_action: AclAction) -> bool {
        match rule_action {
            AclAction::Both => true,
            _ => rule_action == requested_action,
        }
    }

    /// 模式匹配（支持通配符 *）
    fn matches_pattern(&self, pattern: &str, value: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') {
            // 简单的通配符匹配
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return value.starts_with(prefix) && value.ends_with(suffix);
            }
        }

        pattern == value
    }

    /// 添加规则
    pub fn add_rule(&mut self, rule: AclRule) {
        let mut rules = (*self.rules).clone();
        rules.push(rule);
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        self.rules = Arc::new(rules);
        info!("ACL rule added");
    }

    /// 获取规则数量
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for MqttAcl {
    fn default() -> Self {
        Self::new(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_publish_permission() {
        let rules = vec![
            AclRule {
                client_id: Some("sensor_*".to_string()),
                username: None,
                topic_pattern: "sensor/+/data".to_string(),
                action: AclAction::Publish,
                permission: AclPermission::Allow,
                priority: 10,
            },
        ];

        let acl = MqttAcl::new(rules);
        assert!(acl.check_publish("sensor_001", None, "sensor/room1/data"));
        assert!(!acl.check_publish("sensor_001", None, "sensor/room1/status"));
    }

    #[test]
    fn test_acl_subscribe_permission() {
        let rules = vec![
            AclRule {
                client_id: None,
                username: Some("admin".to_string()),
                topic_pattern: "#".to_string(),
                action: AclAction::Both,
                permission: AclPermission::Allow,
                priority: 100,
            },
        ];

        let acl = MqttAcl::new(rules);
        assert!(acl.check_subscribe("any_client", Some("admin"), "any/topic"));
        assert!(acl.check_publish("any_client", Some("admin"), "any/topic"));
    }

    #[test]
    fn test_acl_priority() {
        let rules = vec![
            AclRule {
                client_id: Some("*".to_string()),
                username: None,
                topic_pattern: "#".to_string(),
                action: AclAction::Both,
                permission: AclPermission::Deny,
                priority: 0,
            },
            AclRule {
                client_id: Some("admin_*".to_string()),
                username: None,
                topic_pattern: "#".to_string(),
                action: AclAction::Both,
                permission: AclPermission::Allow,
                priority: 10,
            },
        ];

        let acl = MqttAcl::new(rules);
        // admin_* 规则优先级更高，应该允许
        assert!(acl.check_publish("admin_001", None, "any/topic"));
        // 其他客户端被拒绝
        assert!(!acl.check_publish("user_001", None, "any/topic"));
    }

    #[test]
    fn test_acl_default_deny() {
        let acl = MqttAcl::default();
        assert!(!acl.check_publish("any_client", None, "any/topic"));
        assert!(!acl.check_subscribe("any_client", None, "any/topic"));
    }
}
