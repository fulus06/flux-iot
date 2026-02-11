use crate::{metrics, AppState};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use flux_types::message::Message;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct EventRequest {
    pub topic: String,
    pub payload: Value,
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub script: String,
}

// Handler for POST /api/v1/event
async fn accept_event(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EventRequest>,
) -> impl IntoResponse {
    // 记录 HTTP 请求
    metrics::record_http_request();
    let start = std::time::Instant::now();

    let msg = Message::new(req.topic, req.payload);
    let msg_id = msg.id.to_string();

    // Publish to Event Bus
    if let Err(e) = state.event_bus.publish(msg) {
        tracing::warn!(
            "Event published but no subscribers: {} (Error: {})",
            msg_id,
            e
        );
    } else {
        tracing::debug!("Event published: {}", msg_id);
        // 记录事件发布成功
        metrics::record_event_published();
    }

    // 记录请求时长
    let duration = start.elapsed().as_secs_f64();
    metrics::record_http_duration(duration);

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "id": msg_id })),
    )
}

pub async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    metrics::record_http_request();
    let start = std::time::Instant::now();

    tracing::info!("Creating rule: {}", req.name);

    // 1. Compile & Validate (and Cache in ScriptEngine)
    if let Err(e) = state.script_engine.compile_script(&req.name, &req.script) {
        tracing::error!("Failed to compile rule {}: {}", req.name, e);
        metrics::record_http_duration(start.elapsed().as_secs_f64());
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Script compilation failed: {}", e) })),
        );
    }

    // 2. Persist to DB
    use flux_core::entity::rules;
    use sea_orm::{ActiveModelTrait, Set};

    let rule = rules::ActiveModel {
        name: Set(req.name.clone()),
        script: Set(req.script.clone()),
        active: Set(true),
        created_at: Set(chrono::Utc::now().timestamp_millis()),
        ..Default::default()
    };

    let result = match rule.insert(&state.db).await {
        Ok(_) => {
            // 更新活跃规则数
            let rule_count = state.script_engine.get_script_ids().len();
            metrics::set_active_rules(rule_count);

            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "status": "created", "name": req.name })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to save rule to DB: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Database error: {}", e) })),
            )
        }
    };

    metrics::record_http_duration(start.elapsed().as_secs_f64());
    result
}

pub async fn reload_rules(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    tracing::info!("Reloading rules from Database...");

    use flux_core::entity::rules;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    // 1. Fetch active rules from DB
    let active_rules = match rules::Entity::find()
        .filter(rules::Column::Active.eq(true))
        .all(&state.db)
        .await
    {
        Ok(rules) => rules,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    // 2. Identify rules to compile and rules to remove
    let db_rule_names: Vec<String> = active_rules.iter().map(|r| r.name.clone()).collect();
    let cached_rule_names = state.script_engine.get_script_ids();

    // Compile/Update active rules
    for rule in active_rules {
        if let Err(e) = state.script_engine.compile_script(&rule.name, &rule.script) {
            tracing::error!("Failed to compile rule {}: {}", rule.name, e);
        } else {
            tracing::info!("Reloaded rule: {}", rule.name);
        }
    }

    // Remove rules that are no longer active or in DB
    for cached_name in cached_rule_names {
        if !db_rule_names.contains(&cached_name) {
            tracing::info!("Removing inactive rule: {}", cached_name);
            state.script_engine.remove_script(&cached_name);
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "reloaded", "count": db_rule_names.len() })),
    )
}

pub async fn list_rules(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let scripts = state.script_engine.get_script_ids();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "rules": scripts })),
    )
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/v1/event", post(accept_event))
        .route("/api/v1/rules", post(create_rule).get(list_rules))
        .route("/api/v1/rules/reload", post(reload_rules))
        .with_state(state)
}
