use crate::AppState;
use flux_core::entity::events;
use sea_orm::{ActiveModelTrait, Set};
use std::sync::Arc;

pub async fn start_storage_worker(state: Arc<AppState>) {
    tracing::info!("Starting Storage Worker...");
    let mut rx = state.event_bus.subscribe();

    loop {
        match rx.recv().await {
            Ok(msg) => {
                // Save to DB
                let event = events::ActiveModel {
                    id: Set(msg.id.to_string()),
                    topic: Set(msg.topic.clone()),
                    payload: Set(msg.payload.clone()),
                    timestamp: Set(msg.timestamp),
                };

                if let Err(e) = event.insert(&state.db).await {
                    tracing::error!("Failed to save event {}: {}", msg.id, e);
                } else {
                    tracing::debug!("Saved event {} to DB", msg.id);
                }
            }
            Err(e) => {
                tracing::error!("Storage worker bus error: {}", e);
                if e.to_string().contains("closed") {
                    break;
                }
            }
        }
    }
}
