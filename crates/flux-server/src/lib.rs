// 导出配置模块供测试使用
pub mod config;
pub mod config_provider;
pub mod config_manager;

use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::watch;

// 重新导出配置类型
pub use config::AppConfig;

// 定义 AppState（供 main.rs 和测试使用）
pub struct AppState {
    pub event_bus: Arc<EventBus>,
    pub plugin_manager: Arc<PluginManager>,
    pub script_engine: Arc<ScriptEngine>,
    pub db: DatabaseConnection,
    pub config_db: Option<DatabaseConnection>,
    pub config: watch::Receiver<AppConfig>,
}

// 为了测试，重新导出 api 模块的关键类型和函数
// 注意：这里需要包含完整的 api 模块代码，因为测试需要访问 create_router
pub mod api {
    use crate::AppState;
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

    // 重新实现 create_router 供测试使用（不包含 metrics）
    pub fn create_router(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/health", get(|| async { "OK" }))
            .route("/api/v1/event", post(accept_event))
            .route("/api/v1/rules", post(create_rule).get(list_rules))
            .route("/api/v1/rules/reload", post(reload_rules))
            .with_state(state)
    }

    async fn accept_event(
        State(state): State<Arc<AppState>>,
        Json(req): Json<EventRequest>,
    ) -> impl IntoResponse {
        let msg = Message::new(req.topic, req.payload);
        let msg_id = msg.id.to_string();

        if let Err(e) = state.event_bus.publish(msg) {
            tracing::warn!(
                "Event published but no subscribers: {} (Error: {})",
                msg_id,
                e
            );
        } else {
            tracing::debug!("Event published: {}", msg_id);
        }

        (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "id": msg_id })),
        )
    }

    async fn create_rule(
        State(state): State<Arc<AppState>>,
        Json(req): Json<CreateRuleRequest>,
    ) -> impl IntoResponse {
        tracing::info!("Creating rule: {}", req.name);

        if let Err(e) = state.script_engine.compile_script(&req.name, &req.script) {
            tracing::error!("Failed to compile rule {}: {}", req.name, e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("Script compilation failed: {}", e) })),
            );
        }

        use flux_core::entity::rules;
        use sea_orm::{ActiveModelTrait, Set};

        let rule = rules::ActiveModel {
            name: Set(req.name.clone()),
            script: Set(req.script.clone()),
            active: Set(true),
            created_at: Set(chrono::Utc::now().timestamp_millis()),
            ..Default::default()
        };

        match rule.insert(&state.db).await {
            Ok(_) => (
                StatusCode::CREATED,
                Json(serde_json::json!({ "status": "created", "name": req.name })),
            ),
            Err(e) => {
                tracing::error!("Failed to save rule to DB: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": format!("Database error: {}", e) })),
                )
            }
        }
    }

    async fn reload_rules(State(state): State<Arc<AppState>>) -> impl IntoResponse {
        tracing::info!("Reloading rules from Database...");

        use flux_core::entity::rules;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        match rules::Entity::find()
            .filter(rules::Column::Active.eq(true))
            .all(&state.db)
            .await
        {
            Ok(active_rules) => {
                for rule in active_rules {
                    if let Err(e) = state.script_engine.compile_script(&rule.name, &rule.script) {
                        tracing::error!("Failed to compile rule '{}': {}", rule.name, e);
                    }
                }
                (
                    StatusCode::OK,
                    Json(serde_json::json!({ "status": "reloaded" })),
                )
            }
            Err(e) => {
                tracing::error!("Failed to load rules from DB: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": format!("Database error: {}", e) })),
                )
            }
        }
    }

    async fn list_rules(State(state): State<Arc<AppState>>) -> impl IntoResponse {
        let scripts = state.script_engine.get_script_ids();
        (
            StatusCode::OK,
            Json(serde_json::json!({ "rules": scripts })),
        )
    }
}

pub use api::create_router;
