use std::sync::Arc;
use crate::AppState;
use flux_types::message::Message;

pub async fn start_rule_worker(state: Arc<AppState>) {
    tracing::info!("Starting Rule Worker...");

    // Pre-compile a demo script
    // In reality, we would load this from DB or Config
    let script = r#"
        // Demo Rule: Alert if temperature > 30
        if payload.value > 30.0 {
            print("Alert: High Temperature detected!");
            return true;
        }
        return false;
    "#;
    
    if let Err(e) = state.script_engine.compile_script("rule_demo", script) {
        tracing::error!("Failed to compile demo script: {}", e);
    } else {
        tracing::info!("Demo script 'rule_demo' compiled successfully.");
    }

    // Subscribe to EventBus
    let mut rx = state.event_bus.subscribe();

    loop {
        match rx.recv().await {
            Ok(msg) => {
                tracing::debug!("Worker received message: {}", msg.id);
                // Execute Rule
                match state.script_engine.eval_message("rule_demo", &msg) {
                    Ok(triggered) => {
                        if triggered {
                            tracing::warn!("!!! RULE TRIGGERED for msg {} !!!", msg.id);
                        } else {
                            tracing::debug!("Rule evaluated: no trigger");
                        }
                    },
                    Err(e) => {
                        tracing::error!("Script execution failed: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Bus subscription error: {}", e);
                // If lagged, we might want to continue. use RecvError::Lagged handling if needed.
                // For broadcast, RecvError::Lagged means we missed messages.
                // RecvError::Closed means bus is closed.
                if e.to_string().contains("closed") {
                    break;
                }
            }
        }
    }
}
