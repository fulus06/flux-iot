use super::model::{Scene, SceneTrigger};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// 触发器管理器
pub struct TriggerManager {
    /// 场景列表
    scenes: Arc<RwLock<HashMap<String, Scene>>>,
    
    /// 定时任务句柄
    #[allow(dead_code)]
    scheduler_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl TriggerManager {
    pub fn new() -> Self {
        Self {
            scenes: Arc::new(RwLock::new(HashMap::new())),
            scheduler_handles: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 注册场景
    pub async fn register_scene(&self, scene: Scene) {
        let scene_id = scene.id.clone();
        self.scenes.write().await.insert(scene_id.clone(), scene.clone());
        
        info!(scene_id = %scene_id, "Scene registered");
        
        // 为场景设置触发器
        self.setup_triggers(&scene).await;
    }

    /// 取消注册场景
    pub async fn unregister_scene(&self, scene_id: &str) {
        self.scenes.write().await.remove(scene_id);
        info!(scene_id = %scene_id, "Scene unregistered");
        
        // TODO: 取消该场景的所有触发器
    }

    /// 获取场景
    pub async fn get_scene(&self, scene_id: &str) -> Option<Scene> {
        self.scenes.read().await.get(scene_id).cloned()
    }

    /// 列出所有场景
    pub async fn list_scenes(&self) -> Vec<Scene> {
        self.scenes.read().await.values().cloned().collect()
    }

    /// 设置触发器
    async fn setup_triggers(&self, scene: &Scene) {
        for trigger in &scene.triggers {
            match trigger {
                SceneTrigger::Manual => {
                    debug!(scene_id = %scene.id, "Manual trigger registered");
                }
                SceneTrigger::Schedule { cron } => {
                    debug!(scene_id = %scene.id, cron = %cron, "Schedule trigger registered");
                    // TODO: 实现 Cron 调度
                }
                SceneTrigger::DeviceEvent { device_id, event_type } => {
                    debug!(
                        scene_id = %scene.id,
                        device_id = %device_id,
                        event_type = %event_type,
                        "Device event trigger registered"
                    );
                    // TODO: 订阅设备事件
                }
                SceneTrigger::MetricChange { device_id, metric, operator, threshold } => {
                    debug!(
                        scene_id = %scene.id,
                        device_id = %device_id,
                        metric = %metric,
                        operator = ?operator,
                        threshold = %threshold,
                        "Metric change trigger registered"
                    );
                    // TODO: 监控指标变化
                }
                SceneTrigger::StatusChange { device_id, from_status, to_status } => {
                    debug!(
                        scene_id = %scene.id,
                        device_id = %device_id,
                        from_status = ?from_status,
                        to_status = %to_status,
                        "Status change trigger registered"
                    );
                    // TODO: 监控状态变化
                }
            }
        }
    }

    /// 手动触发场景
    pub async fn trigger_scene(&self, scene_id: &str) -> Option<Scene> {
        self.get_scene(scene_id).await
    }
}

impl Default for TriggerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::model::ComparisonOperator;

    #[tokio::test]
    async fn test_register_scene() {
        let manager = TriggerManager::new();
        
        let scene = Scene::new(
            "测试场景".to_string(),
            "log('test')".to_string(),
        )
        .with_trigger(SceneTrigger::Manual);
        
        let scene_id = scene.id.clone();
        manager.register_scene(scene).await;
        
        let retrieved = manager.get_scene(&scene_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "测试场景");
    }

    #[tokio::test]
    async fn test_list_scenes() {
        let manager = TriggerManager::new();
        
        let scene1 = Scene::new("场景1".to_string(), "log('1')".to_string());
        let scene2 = Scene::new("场景2".to_string(), "log('2')".to_string());
        
        manager.register_scene(scene1).await;
        manager.register_scene(scene2).await;
        
        let scenes = manager.list_scenes().await;
        assert_eq!(scenes.len(), 2);
    }

    #[tokio::test]
    async fn test_unregister_scene() {
        let manager = TriggerManager::new();
        
        let scene = Scene::new("测试".to_string(), "log('test')".to_string());
        let scene_id = scene.id.clone();
        
        manager.register_scene(scene).await;
        assert!(manager.get_scene(&scene_id).await.is_some());
        
        manager.unregister_scene(&scene_id).await;
        assert!(manager.get_scene(&scene_id).await.is_none());
    }

    #[tokio::test]
    async fn test_trigger_types() {
        let manager = TriggerManager::new();
        
        let scene = Scene::new(
            "复杂场景".to_string(),
            "log('complex')".to_string(),
        )
        .with_trigger(SceneTrigger::Manual)
        .with_trigger(SceneTrigger::Schedule {
            cron: "0 0 * * *".to_string(),
        })
        .with_trigger(SceneTrigger::MetricChange {
            device_id: "sensor_01".to_string(),
            metric: "temperature".to_string(),
            operator: ComparisonOperator::GreaterThan,
            threshold: 30.0,
        });
        
        manager.register_scene(scene).await;
        
        let scenes = manager.list_scenes().await;
        assert_eq!(scenes[0].triggers.len(), 3);
    }
}
