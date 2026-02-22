use crate::model::Rule;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 规则存储（内存实现）
pub struct RuleStorage {
    rules: Arc<RwLock<HashMap<String, Rule>>>,
}

impl RuleStorage {
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn save(&self, rule: Rule) -> anyhow::Result<()> {
        let mut rules = self.rules.write().await;
        rules.insert(rule.id.clone(), rule);
        Ok(())
    }
    
    pub async fn get(&self, rule_id: &str) -> anyhow::Result<Option<Rule>> {
        let rules = self.rules.read().await;
        Ok(rules.get(rule_id).cloned())
    }
    
    pub async fn delete(&self, rule_id: &str) -> anyhow::Result<()> {
        let mut rules = self.rules.write().await;
        rules.remove(rule_id);
        Ok(())
    }
    
    pub async fn list(&self) -> anyhow::Result<Vec<Rule>> {
        let rules = self.rules.read().await;
        Ok(rules.values().cloned().collect())
    }
    
    pub async fn find_by_group(&self, group: &str) -> anyhow::Result<Vec<Rule>> {
        let rules = self.rules.read().await;
        Ok(rules.values()
            .filter(|r| r.group.as_deref() == Some(group))
            .cloned()
            .collect())
    }
    
    pub async fn find_by_tag(&self, tag: &str) -> anyhow::Result<Vec<Rule>> {
        let rules = self.rules.read().await;
        Ok(rules.values()
            .filter(|r| r.tags.contains(&tag.to_string()))
            .cloned()
            .collect())
    }
}

impl Default for RuleStorage {
    fn default() -> Self {
        Self::new()
    }
}
