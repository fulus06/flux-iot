use crate::AppState;
use flux_core::entity::events;
use sea_orm::{ActiveModelTrait, Set};
use std::sync::Arc;
use tokio::time::interval;

pub async fn start_storage_worker(state: Arc<AppState>) {
    tracing::info!("Starting Storage Worker...");
    let mut rx = state.event_bus.subscribe();

    loop {
        match rx.recv().await {
            Ok(msg) => {
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

pub async fn start_storage_metrics_worker(state: Arc<AppState>) {
    let mut ticker = interval(std::time::Duration::from_secs(30));

    loop {
        ticker.tick().await;

        if let Err(e) = state.storage_manager.refresh().await {
            tracing::warn!(target: "flux_server", "StorageManager refresh failed: {}", e);
        }

        let metrics = state.storage_manager.get_metrics().await;
        crate::metrics::set_storage_metrics(&metrics);

        let pools = state.storage_manager.get_pools_stats().await;
        for p in pools {
            crate::metrics::set_storage_pool_stats(&p);
        }
    }
}
