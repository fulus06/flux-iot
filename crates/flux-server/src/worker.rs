use std::sync::Arc;
use crate::AppState;

pub async fn start_rule_worker(state: Arc<AppState>) {
    tracing::info!("Starting Rule Worker...");

    // Load rules from DB
    use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
    use flux_core::entity::rules;
    
    match rules::Entity::find()
        .filter(rules::Column::Active.eq(true))
        .all(&state.db)
        .await 
    {
        Ok(active_rules) => {
            for rule in active_rules {
                tracing::info!("Compiling rule: {}", rule.name);
                if let Err(e) = state.script_engine.compile_script(&rule.name, &rule.script) {
                    tracing::error!("Failed to compile rule '{}': {}", rule.name, e);
                }
            }
        },
        Err(e) => tracing::error!("Failed to load rules from DB: {}", e),
    }

    // Subscribe to EventBus
    let mut rx = state.event_bus.subscribe();

    loop {
        match rx.recv().await {
            Ok(msg) => {
                tracing::debug!("Worker received message: {}", msg.id);
                
                // 🔥 阶段 1: 插件预处理
                // 将消息序列化为 JSON 传递给插件
                let msg_json = match serde_json::to_string(&msg) {
                    Ok(json) => json,
                    Err(e) => {
                        tracing::error!("Failed to serialize message: {}", e);
                        continue;
                    }
                };
                
                // 调用所有已加载的插件进行预处理
                // 注意：这里简化处理，实际可以配置每个规则使用哪些插件
                tracing::debug!("Calling plugins for message preprocessing");
                
                // 示例：调用 dummy_plugin 的 on_msg 函数
                // 返回值是处理后的消息长度（示例插件的简单逻辑）
                match state.plugin_manager.call_plugin("dummy_plugin", "on_msg", &msg_json) {
                    Ok(result) => {
                        tracing::info!("Plugin 'dummy_plugin' processed message, result: {}", result);
                        // 实际应用中，插件可能返回修改后的 JSON，这里简化处理
                    },
                    Err(e) => {
                        // 插件失败不应该阻止规则执行
                        tracing::warn!("Plugin 'dummy_plugin' failed: {}, continuing with original message", e);
                    }
                }
                
                // 🔥 阶段 2: 规则引擎执行
                // 注意：这里使用原始消息，实际应用中应该使用插件处理后的消息
                let script_ids = state.script_engine.get_script_ids();
                for script_id in script_ids {
                    match state.script_engine.eval_message(&script_id, &msg) {
                        Ok(triggered) => {
                             if triggered {
                                 tracing::warn!("!!! RULE TRIGGERED: {} (msg {}) !!!", script_id, msg.id);
                                 
                                 // 🔥 阶段 3: 规则触发后的动作插件（可选）
                                 // 这里可以调用动作插件，例如发送通知、控制设备等
                                 tracing::info!("Rule '{}' triggered, executing actions...", script_id);
                             }
                        },
                        Err(e) => {
                            tracing::error!("Failed to execute rule {}: {}", script_id, e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Bus subscription error: {}", e);
                if e.to_string().contains("closed") {
                    break;
                }
            }
        }
    }
}
