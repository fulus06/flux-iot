use crate::context::RuleContext;
use crate::execution::{ExecutionStatus, RuleExecution, TestResult};
use crate::model::Rule;
use crate::storage::RuleStorage;
use anyhow::Result;
use chrono::Utc;
use flux_script::ScriptEngine;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// 规则引擎
pub struct RuleEngine {
    /// Rhai 脚本引擎
    script_engine: Arc<ScriptEngine>,
    
    /// 规则存储
    storage: Arc<RuleStorage>,
    
    /// 执行历史
    executions: Arc<RwLock<Vec<RuleExecution>>>,
    
    /// 限流计数器 (rule_id -> (timestamp, count))
    rate_limit_counters: Arc<RwLock<HashMap<String, Vec<i64>>>>,
}

impl RuleEngine {
    pub fn new() -> Self {
        let script_engine = ScriptEngine::new();
        
        Self {
            script_engine: Arc::new(script_engine),
            storage: Arc::new(RuleStorage::new()),
            executions: Arc::new(RwLock::new(Vec::new())),
            rate_limit_counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 添加规则
    pub async fn add_rule(&self, mut rule: Rule) -> Result<String> {
        // 验证脚本语法
        self.script_engine.compile(&rule.script)?;
        
        // 生成 ID（如果没有）
        if rule.id.is_empty() {
            rule.id = uuid::Uuid::new_v4().to_string();
        }
        
        // 保存规则
        self.storage.save(rule.clone()).await?;
        
        info!(rule_id = %rule.id, rule_name = %rule.name, "Rule added");
        
        Ok(rule.id)
    }
    
    /// 获取规则
    pub async fn get_rule(&self, rule_id: &str) -> Result<Rule> {
        self.storage.get(rule_id).await?
            .ok_or_else(|| anyhow::anyhow!("Rule not found: {}", rule_id))
    }
    
    /// 删除规则
    pub async fn delete_rule(&self, rule_id: &str) -> Result<()> {
        self.storage.delete(rule_id).await?;
        info!(rule_id = %rule_id, "Rule deleted");
        Ok(())
    }
    
    /// 列出所有规则
    pub async fn list_rules(&self) -> Result<Vec<Rule>> {
        self.storage.list().await
    }
    
    /// 手动触发规则
    pub async fn trigger_manual(&self, rule_id: &str, context: RuleContext) -> Result<()> {
        let rule = self.get_rule(rule_id).await?;
        
        if !rule.enabled {
            return Err(anyhow::anyhow!("Rule is disabled"));
        }
        
        self.execute_rule(&rule, context).await
    }
    
    /// 执行规则
    async fn execute_rule(&self, rule: &Rule, context: RuleContext) -> Result<()> {
        // 检查限流
        if let Some(rate_limit) = &rule.rate_limit {
            if !self.check_rate_limit(&rule.id, rate_limit).await {
                warn!(rule_id = %rule.id, "Rate limit exceeded");
                return Err(anyhow::anyhow!("Rate limit exceeded"));
            }
        }
        
        // 创建执行记录
        let execution_id = uuid::Uuid::new_v4().to_string();
        let mut execution = RuleExecution {
            id: execution_id.clone(),
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            trigger_type: format!("{:?}", rule.trigger),
            started_at: Utc::now(),
            finished_at: None,
            status: ExecutionStatus::Running,
            error: None,
            context: serde_json::to_value(&context).unwrap_or_default(),
        };
        
        // 保存执行记录
        {
            let mut executions = self.executions.write().await;
            executions.push(execution.clone());
        }
        
        // 执行脚本（带超时）
        let timeout = Duration::from_secs(rule.timeout_seconds);
        let script = rule.script.clone();
        let engine = self.script_engine.clone();
        
        let result = tokio::time::timeout(timeout, async move {
            // 准备脚本上下文
            let mut scope = rhai::Scope::new();
            
            // 注入设备数据
            scope.push("device", context.device_data.clone());
            
            // 注入系统变量
            scope.push("system", context.system_vars.clone());
            
            // 注入参数
            scope.push("params", rule.parameters.clone());
            
            // 执行脚本
            engine.eval_with_scope(&mut scope, &script)
        }).await;
        
        // 更新执行记录
        execution.finished_at = Some(Utc::now());
        
        match result {
            Ok(Ok(_)) => {
                execution.status = ExecutionStatus::Success;
                debug!(rule_id = %rule.id, "Rule executed successfully");
            }
            Ok(Err(e)) => {
                execution.status = ExecutionStatus::Failed;
                execution.error = Some(e.to_string());
                error!(rule_id = %rule.id, error = %e, "Rule execution failed");
            }
            Err(_) => {
                execution.status = ExecutionStatus::Timeout;
                execution.error = Some("Execution timeout".to_string());
                warn!(rule_id = %rule.id, "Rule execution timeout");
            }
        }
        
        // 更新执行历史
        {
            let mut executions = self.executions.write().await;
            if let Some(exec) = executions.iter_mut().find(|e| e.id == execution_id) {
                *exec = execution.clone();
            }
        }
        
        // 记录限流计数
        if rule.rate_limit.is_some() {
            self.record_execution(&rule.id).await;
        }
        
        if execution.status == ExecutionStatus::Success {
            Ok(())
        } else {
            Err(anyhow::anyhow!("{}", execution.error.unwrap_or_default()))
        }
    }
    
    /// 测试规则（不实际执行动作）
    pub async fn test_rule(&self, rule_id: &str, mock_context: RuleContext) -> Result<TestResult> {
        let rule = self.get_rule(rule_id).await?;
        
        let start = std::time::Instant::now();
        let mut logs = Vec::new();
        let mut actions = Vec::new();
        
        // 准备测试环境
        let mut scope = rhai::Scope::new();
        scope.push("device", mock_context.device_data.clone());
        scope.push("system", mock_context.system_vars.clone());
        scope.push("params", rule.parameters.clone());
        
        // 执行脚本
        let result = self.script_engine.eval_with_scope(&mut scope, &rule.script);
        
        let duration_ms = start.elapsed().as_millis() as u64;
        
        match result {
            Ok(_) => {
                logs.push("Script executed successfully".to_string());
                Ok(TestResult {
                    success: true,
                    logs,
                    actions,
                    error: None,
                    duration_ms,
                })
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                Ok(TestResult {
                    success: false,
                    logs,
                    actions,
                    error: Some(error_msg),
                    duration_ms,
                })
            }
        }
    }
    
    /// 获取执行历史
    pub async fn get_execution_history(&self, rule_id: Option<&str>, limit: usize) -> Result<Vec<RuleExecution>> {
        let executions = self.executions.read().await;
        
        let mut filtered: Vec<_> = if let Some(rid) = rule_id {
            executions.iter()
                .filter(|e| e.rule_id == rid)
                .cloned()
                .collect()
        } else {
            executions.clone()
        };
        
        // 按时间倒序
        filtered.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        
        // 限制数量
        filtered.truncate(limit);
        
        Ok(filtered)
    }
    
    /// 检查限流
    async fn check_rate_limit(&self, rule_id: &str, rate_limit: &crate::model::RateLimit) -> bool {
        let mut counters = self.rate_limit_counters.write().await;
        let now = Utc::now().timestamp();
        let window_start = now - rate_limit.time_window_seconds as i64;
        
        // 获取或创建计数器
        let timestamps = counters.entry(rule_id.to_string()).or_insert_with(Vec::new);
        
        // 清理过期的时间戳
        timestamps.retain(|&ts| ts > window_start);
        
        // 检查是否超过限制
        timestamps.len() < rate_limit.max_executions as usize
    }
    
    /// 记录执行（用于限流）
    async fn record_execution(&self, rule_id: &str) {
        let mut counters = self.rate_limit_counters.write().await;
        let now = Utc::now().timestamp();
        
        counters.entry(rule_id.to_string())
            .or_insert_with(Vec::new)
            .push(now);
    }
    
    /// 按分组启用/禁用规则
    pub async fn enable_group(&self, group: &str, enabled: bool) -> Result<usize> {
        let rules = self.storage.find_by_group(group).await?;
        let mut count = 0;
        
        for mut rule in rules {
            rule.enabled = enabled;
            self.storage.save(rule).await?;
            count += 1;
        }
        
        info!(group = %group, enabled = %enabled, count = %count, "Group rules updated");
        Ok(count)
    }
    
    /// 按标签查找规则
    pub async fn find_by_tag(&self, tag: &str) -> Result<Vec<Rule>> {
        self.storage.find_by_tag(tag).await
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RuleTrigger;

    #[tokio::test]
    async fn test_add_and_get_rule() {
        let engine = RuleEngine::new();
        
        let rule = Rule {
            name: "test_rule".to_string(),
            script: "let x = 1 + 1;".to_string(),
            trigger: RuleTrigger::Manual,
            ..Default::default()
        };
        
        let rule_id = engine.add_rule(rule).await.unwrap();
        let retrieved = engine.get_rule(&rule_id).await.unwrap();
        
        assert_eq!(retrieved.name, "test_rule");
    }

    #[tokio::test]
    async fn test_rate_limit() {
        let engine = RuleEngine::new();
        
        let rule = Rule {
            name: "rate_limited_rule".to_string(),
            script: "let x = 1;".to_string(),
            trigger: RuleTrigger::Manual,
            rate_limit: Some(crate::model::RateLimit {
                max_executions: 2,
                time_window_seconds: 60,
            }),
            ..Default::default()
        };
        
        let rule_id = engine.add_rule(rule).await.unwrap();
        let context = RuleContext::new();
        
        // 前两次应该成功
        assert!(engine.trigger_manual(&rule_id, context.clone()).await.is_ok());
        assert!(engine.trigger_manual(&rule_id, context.clone()).await.is_ok());
        
        // 第三次应该失败（超过限流）
        assert!(engine.trigger_manual(&rule_id, context).await.is_err());
    }
}

