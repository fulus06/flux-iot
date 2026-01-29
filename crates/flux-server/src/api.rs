use axum::{
    routing::{post, get},
    Router, Json, extract::State, response::IntoResponse,
    http::StatusCode,
};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::Value;
use flux_types::message::Message;
use crate::AppState;

#[derive(Deserialize)]
pub struct EventRequest {
    pub topic: String,
    pub payload: Value,
}

// Handler for POST /api/v1/event
async fn accept_event(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EventRequest>,
) -> impl IntoResponse {
    let msg = Message::new(req.topic, req.payload);
    let msg_id = msg.id.to_string();
    
    // Publish to Event Bus
    if let Err(e) = state.event_bus.publish(msg) {
        // Log error but mostly we don't care if there are no subscribers yet
        tracing::warn!("Event published but no subscribers: {} (Error: {})", msg_id, e);
    } else {
        tracing::debug!("Event published: {}", msg_id);
    }
    
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok", "id": msg_id })))
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/v1/event", post(accept_event))
        .with_state(state)
}
