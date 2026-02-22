use super::model::{Scene, SceneExecution};
use crate::command::{CommandExecutor, CommandType, DeviceCommand};
use anyhow::Result;
use chrono::{Datelike, Timelike, Utc};
use rhai::{Engine, Scope, AST};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// 场景引擎
pub struct SceneEngine {
    /// Rhai 引擎
    engine: Engine,
    
    /// 编译后的脚本缓存
    script_cache: Arc<RwLock<HashMap<String, AST>>>,
    
    /// 指令执行器
    command_executor: Arc<CommandExecutor>,
    
    /// 设备状态缓存（用于脚本查询）
    device_states: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl SceneEngine {
    pub fn new(command_executor: Arc<CommandExecutor>) -> Self {
        let mut engine = Engine::new();
        
        // 安全限制
        engine.set_max_operations(100_000);
        engine.set_max_expr_depths(50, 50);
        
        // 注册日志函数
        engine.on_print(|x| {
            info!("SCENE: {}", x);
        });
        
        engine.on_debug(|x, src, pos| {
            debug!(source = ?src, position = ?pos, "SCENE DEBUG: {}", x);
        });
        
        Self {
            engine,
            script_cache: Arc::new(RwLock::new(HashMap::new())),
            command_executor,
            device_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册设备控制函数
    pub fn register_device_functions(&mut self) {
        let executor = self.command_executor.clone();
        
        // 注册 send_command 函数
        self.engine.register_fn(
            "send_command",
            move |device_id: &str, cmd_type: &str, params: rhai::Map| -> bool {
                let executor = executor.clone();
                let device_id = device_id.to_string();
                let cmd_type = cmd_type.to_string();
                
                // 将 Rhai Map 转换为 JSON
                let params_json = match rhai::serde::to_dynamic(&params) {
                    Ok(dynamic) => match serde_json::to_value(&dynamic) {
                        Ok(json) => json,
                        Err(e) => {
                            error!(error = %e, "Failed to convert params to JSON");
                            return false;
                        }
                    },
                    Err(e) => {
                        error!(error = %e, "Failed to convert params");
                        return false;
                    }
                };
                
                // 创建指令
                let command_type = match cmd_type.as_str() {
                    "reboot" => CommandType::Reboot,
                    "reset" => CommandType::Reset,
                    "set_state" => {
                        if let Some(state) = params_json.get("state").and_then(|v| v.as_bool()) {
                            CommandType::SetState { state }
                        } else {
                            error!("Invalid state parameter");
                            return false;
                        }
                    }
                    "set_value" => {
                        if let Some(value) = params_json.get("value").and_then(|v| v.as_f64()) {
                            CommandType::SetValue { value }
                        } else {
                            error!("Invalid value parameter");
                            return false;
                        }
                    }
                    _ => CommandType::Custom {
                        name: cmd_type.clone(),
                        params: params_json.clone(),
                    },
                };
                
                let command = DeviceCommand::new(device_id, command_type);
                
                // 异步提交指令
                tokio::spawn(async move {
                    match executor.submit(command.clone()).await {
                        Ok(cmd_id) => {
                            info!(command_id = %cmd_id, "Scene command submitted");
                            // 异步执行
                            if let Err(e) = executor.execute(command).await {
                                error!(error = %e, "Failed to execute scene command");
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to submit scene command");
                        }
                    }
                });
                
                true
            },
        );
    }

    /// 注册设备查询函数
    pub fn register_query_functions(&mut self) {
        let states = self.device_states.clone();
        
        // 注册 get_device_status 函数
        self.engine.register_fn(
            "get_device_status",
            move |device_id: &str| -> String {
                let states = states.clone();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let states_lock = states.read().await;
                        states_lock
                            .get(device_id)
                            .and_then(|v| v.get("status"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string()
                    })
                })
            },
        );
        
        let states = self.device_states.clone();
        
        // 注册 get_metric 函数
        self.engine.register_fn(
            "get_metric",
            move |device_id: &str, metric: &str| -> f64 {
                let states = states.clone();
                let metric = metric.to_string();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let states_lock = states.read().await;
                        states_lock
                            .get(device_id)
                            .and_then(|v| v.get("metrics"))
                            .and_then(|v| v.get(&metric))
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0)
                    })
                })
            },
        );
    }

    /// 注册时间函数
    pub fn register_time_functions(&mut self) {
        self.engine.register_fn("get_hour", || {
            chrono::Local::now().hour() as i64
        });
        
        self.engine.register_fn("get_minute", || {
            chrono::Local::now().minute() as i64
        });
        
        self.engine.register_fn("get_day_of_week", || {
            chrono::Local::now().weekday().num_days_from_monday() as i64
        });
        
        self.engine.register_fn("is_weekend", || {
            let day = chrono::Local::now().weekday().num_days_from_monday();
            day >= 5 // Saturday or Sunday
        });
    }

    /// 注册通知函数
    pub fn register_notification_functions(&mut self) {
        self.engine.register_fn("send_notification", |message: &str| {
            info!("NOTIFICATION: {}", message);
            // TODO: 集成实际的通知系统
        });
        
        self.engine.register_fn("log", |message: &str| {
            info!("SCENE LOG: {}", message);
        });
    }

    /// 编译场景脚本
    pub async fn compile_scene(&self, scene: &Scene) -> Result<()> {
        let mut cache = self.script_cache.write().await;
        
        // 编译条件脚本
        if let Some(condition) = &scene.condition_script {
            let ast = self.engine.compile(condition)?;
            cache.insert(format!("{}_condition", scene.id), ast);
        }
        
        // 编译动作脚本
        let ast = self.engine.compile(&scene.action_script)?;
        cache.insert(format!("{}_action", scene.id), ast);
        
        info!(scene_id = %scene.id, "Scene compiled successfully");
        Ok(())
    }

    /// 执行场景
    pub async fn execute_scene(&self, scene: &Scene) -> Result<SceneExecution> {
        let start_time = Instant::now();
        let mut execution = SceneExecution {
            id: 0, // Will be set by database
            scene_id: scene.id.clone(),
            trigger_type: "manual".to_string(),
            executed_at: Utc::now(),
            success: false,
            error: None,
            duration_ms: None,
        };

        // 检查是否启用
        if !scene.enabled {
            warn!(scene_id = %scene.id, "Scene is disabled");
            execution.error = Some("Scene is disabled".to_string());
            return Ok(execution);
        }

        // 执行条件检查
        if let Some(_condition) = &scene.condition_script {
            let cache = self.script_cache.read().await;
            let condition_key = format!("{}_condition", scene.id);
            
            if let Some(ast) = cache.get(&condition_key) {
                let mut scope = Scope::new();
                
                match self.engine.eval_ast_with_scope::<bool>(&mut scope, ast) {
                    Ok(result) => {
                        if !result {
                            debug!(scene_id = %scene.id, "Condition not met, skipping execution");
                            execution.success = true;
                            execution.duration_ms = Some(start_time.elapsed().as_millis() as i64);
                            return Ok(execution);
                        }
                    }
                    Err(e) => {
                        error!(scene_id = %scene.id, error = %e, "Condition evaluation failed");
                        execution.error = Some(format!("Condition error: {}", e));
                        execution.duration_ms = Some(start_time.elapsed().as_millis() as i64);
                        return Ok(execution);
                    }
                }
            }
        }

        // 执行动作脚本
        let cache = self.script_cache.read().await;
        let action_key = format!("{}_action", scene.id);
        
        if let Some(ast) = cache.get(&action_key) {
            let mut scope = Scope::new();
            
            match self.engine.eval_ast_with_scope::<()>(&mut scope, ast) {
                Ok(_) => {
                    info!(scene_id = %scene.id, "Scene executed successfully");
                    execution.success = true;
                }
                Err(e) => {
                    error!(scene_id = %scene.id, error = %e, "Action execution failed");
                    execution.error = Some(format!("Action error: {}", e));
                }
            }
        } else {
            execution.error = Some("Scene not compiled".to_string());
        }

        execution.duration_ms = Some(start_time.elapsed().as_millis() as i64);
        Ok(execution)
    }

    /// 更新设备状态（供外部调用）
    pub async fn update_device_state(&self, device_id: String, state: serde_json::Value) {
        self.device_states.write().await.insert(device_id, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::MockCommandChannel;

    #[tokio::test]
    async fn test_scene_engine_creation() {
        let channel = Arc::new(MockCommandChannel::new());
        let executor = Arc::new(CommandExecutor::new(channel));
        let mut engine = SceneEngine::new(executor);
        
        engine.register_device_functions();
        engine.register_query_functions();
        engine.register_time_functions();
        engine.register_notification_functions();
        
        // Engine should be ready
        assert!(true);
    }

    #[tokio::test]
    async fn test_compile_and_execute_scene() {
        let channel = Arc::new(MockCommandChannel::new());
        let executor = Arc::new(CommandExecutor::new(channel));
        let mut engine = SceneEngine::new(executor);
        
        engine.register_device_functions();
        engine.register_notification_functions();
        
        let scene = Scene::new(
            "测试场景".to_string(),
            r#"log("Scene executed")"#.to_string(),
        );
        
        engine.compile_scene(&scene).await.unwrap();
        let result = engine.execute_scene(&scene).await.unwrap();
        
        assert!(result.success);
        assert!(result.error.is_none());
    }
}
