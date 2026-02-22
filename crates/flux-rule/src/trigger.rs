use crate::context::RuleContext;
use crate::engine::RuleEngine;
use crate::model::{Rule, RuleTrigger};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info};

/// 触发器管理器
pub struct TriggerManager {
    engine: Arc<RuleEngine>,
    scheduler: Arc<RwLock<Option<JobScheduler>>>,
}

impl TriggerManager {
    pub fn new(engine: Arc<RuleEngine>) -> Self {
        Self {
            engine,
            scheduler: Arc::new(RwLock::new(None)),
        }
    }
    
    /// 启动触发器系统
    pub async fn start(&self) -> Result<()> {
        let scheduler = JobScheduler::new().await?;
        scheduler.start().await?;
        
        *self.scheduler.write().await = Some(scheduler);
        
        info!("Trigger manager started");
        Ok(())
    }
    
    /// 停止触发器系统
    pub async fn stop(&self) -> Result<()> {
        if let Some(mut scheduler) = self.scheduler.write().await.take() {
            scheduler.shutdown().await?;
        }
        
        info!("Trigger manager stopped");
        Ok(())
    }
    
    /// 注册规则触发器
    pub async fn register_rule(&self, rule: &Rule) -> Result<()> {
        match &rule.trigger {
            RuleTrigger::Schedule { cron } => {
                self.register_schedule_trigger(rule, cron).await?;
            }
            RuleTrigger::Manual => {
                // 手动触发不需要注册
                debug!(rule_id = %rule.id, "Manual trigger, no registration needed");
            }
            RuleTrigger::DeviceEvent { device_id, event_type } => {
                debug!(
                    rule_id = %rule.id,
                    device_id = %device_id,
                    event_type = %event_type,
                    "Device event trigger registered (requires external event bus)"
                );
            }
            RuleTrigger::DataChange { device_id, metric } => {
                debug!(
                    rule_id = %rule.id,
                    device_id = %device_id,
                    metric = ?metric,
                    "Data change trigger registered (requires external event bus)"
                );
            }
        }
        
        Ok(())
    }
    
    /// 注册定时触发器
    async fn register_schedule_trigger(&self, rule: &Rule, cron: &str) -> Result<()> {
        let scheduler_lock = self.scheduler.read().await;
        let scheduler = scheduler_lock.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Scheduler not started"))?;
        
        let rule_id = rule.id.clone();
        let rule_name = rule.name.clone();
        let engine = self.engine.clone();
        
        let job = Job::new_async(cron, move |_uuid, _lock| {
            let rule_id = rule_id.clone();
            let rule_name = rule_name.clone();
            let engine = engine.clone();
            
            Box::pin(async move {
                info!(rule_id = %rule_id, rule_name = %rule_name, "Executing scheduled rule");
                
                let context = RuleContext::new();
                if let Err(e) = engine.trigger_manual(&rule_id, context).await {
                    error!(rule_id = %rule_id, error = %e, "Failed to execute scheduled rule");
                }
            })
        })?;
        
        scheduler.add(job).await?;
        
        info!(
            rule_id = %rule.id,
            rule_name = %rule.name,
            cron = %cron,
            "Schedule trigger registered"
        );
        
        Ok(())
    }
    
    /// 处理设备事件（外部调用）
    pub async fn handle_device_event(
        &self,
        device_id: &str,
        event_type: &str,
        data: serde_json::Value,
    ) -> Result<()> {
        // 查找匹配的规则
        let rules = self.engine.list_rules().await?;
        
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            
            if let RuleTrigger::DeviceEvent {
                device_id: trigger_device_id,
                event_type: trigger_event_type,
            } = &rule.trigger
            {
                if trigger_device_id == device_id && trigger_event_type == event_type {
                    let mut context = RuleContext::new();
                    context.device_data.insert("event_data".to_string(), data.clone());
                    context.device_data.insert("device_id".to_string(), device_id.into());
                    context.device_data.insert("event_type".to_string(), event_type.into());
                    
                    info!(
                        rule_id = %rule.id,
                        device_id = %device_id,
                        event_type = %event_type,
                        "Triggering rule by device event"
                    );
                    
                    if let Err(e) = self.engine.trigger_manual(&rule.id, context).await {
                        error!(rule_id = %rule.id, error = %e, "Failed to execute rule");
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 处理数据变化（外部调用）
    pub async fn handle_data_change(
        &self,
        device_id: &str,
        metric: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        // 查找匹配的规则
        let rules = self.engine.list_rules().await?;
        
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            
            if let RuleTrigger::DataChange {
                device_id: trigger_device_id,
                metric: trigger_metric,
            } = &rule.trigger
            {
                // 检查设备 ID 是否匹配
                if trigger_device_id != device_id {
                    continue;
                }
                
                // 检查指标是否匹配（None 表示任何指标）
                if let Some(tm) = trigger_metric {
                    if tm != metric {
                        continue;
                    }
                }
                
                let mut context = RuleContext::new();
                context.device_data.insert(metric.to_string(), value.clone());
                context.device_data.insert("device_id".to_string(), device_id.into());
                
                info!(
                    rule_id = %rule.id,
                    device_id = %device_id,
                    metric = %metric,
                    "Triggering rule by data change"
                );
                
                if let Err(e) = self.engine.trigger_manual(&rule.id, context).await {
                    error!(rule_id = %rule.id, error = %e, "Failed to execute rule");
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rule;

    #[tokio::test]
    async fn test_trigger_manager() {
        let engine = Arc::new(RuleEngine::new());
        let manager = TriggerManager::new(engine.clone());
        
        manager.start().await.unwrap();
        
        // 测试定时触发器注册
        let rule = Rule {
            name: "test_schedule".to_string(),
            trigger: RuleTrigger::Schedule {
                cron: "*/5 * * * * *".to_string(), // 每5秒
            },
            script: "let x = 1;".to_string(),
            ..Default::default()
        };
        
        manager.register_rule(&rule).await.unwrap();
        
        manager.stop().await.unwrap();
    }
}
