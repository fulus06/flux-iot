use std::sync::Arc;
use crate::AppState;
use flux_types::message::Message;

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
                
                // Dynamic Rule Execution
                let script_ids = state.script_engine.get_script_ids();
                for script_id in script_ids {
                    match state.script_engine.eval_message(&script_id, &msg) {
                        Ok(triggered) => {
                             if triggered {
                                 tracing::warn!("!!! RULE TRIGGERED: {} (msg {}) !!!", script_id, msg.id);
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
