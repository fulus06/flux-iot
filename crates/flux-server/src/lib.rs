// 导出配置模块供测试使用
pub mod config;
pub mod config_provider;
pub mod config_manager;
pub mod gb28181_backend;

use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::watch;
use flux_video::gb28181::sip::SipServer;
use crate::gb28181_backend::Gb28181BackendRef;
use flux_storage::StorageManager;

// 重新导出配置类型
pub use config::AppConfig;

// 定义 AppState（供 main.rs 和测试使用）
pub struct AppState {
    pub event_bus: Arc<EventBus>,
    pub plugin_manager: Arc<PluginManager>,
    pub script_engine: Arc<ScriptEngine>,
    pub storage_manager: Arc<StorageManager>,
    pub db: DatabaseConnection,
    pub config_db: Option<DatabaseConnection>,
    pub config: watch::Receiver<AppConfig>,
    pub gb28181_sip: Option<Arc<SipServer>>,
    pub gb28181_backend: Option<Gb28181BackendRef>,
}

// 为了测试，重新导出 api 模块的关键类型和函数
// 注意：这里需要包含完整的 api 模块代码，因为测试需要访问 create_router
pub mod api {
    use crate::AppState;
    use axum::{
        extract::{Query, State},
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use flux_core::entity::events;
    use flux_types::message::Message;
    use serde::Deserialize;
    use serde::Serialize;
    use serde_json::Value;
    use std::sync::Arc;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};

    #[derive(Deserialize)]
    pub struct EventRequest {
        pub topic: String,
        pub payload: Value,
    }

    #[derive(Deserialize)]
    pub struct StorageTelemetryRequest {
        pub topic: String,
        pub payload: Value,
    }

    #[derive(Deserialize)]
    pub struct StorageTelemetryStatsQuery {
        pub topic_prefix: Option<String>,
        pub since: Option<i64>,
        pub until: Option<i64>,
        pub max_rows: Option<u64>,
        pub window_secs: Option<u64>,
    }

    #[derive(Debug, Serialize)]
    pub struct StorageTelemetryStatsItem {
        pub topic: String,
        pub service: String,
        pub count: u64,
    }

    #[derive(Debug, Serialize)]
    pub struct StorageTelemetryStatsResponse {
        pub topic_prefix: String,
        pub since: Option<i64>,
        pub until: Option<i64>,
        pub rows_scanned: u64,
        pub items: Vec<StorageTelemetryStatsItem>,
    }

    #[derive(Deserialize)]
    pub struct StorageTelemetryTroubleshootQuery {
        pub topic_prefix: Option<String>,
        pub window_secs: Option<u64>,
        pub top: Option<usize>,
        pub samples: Option<u64>,
    }

    #[derive(Debug, Serialize)]
    pub struct StorageTelemetryErrorSample {
        pub timestamp: i64,
        pub topic: String,
        pub service: String,
        pub stream_id: Option<String>,
        pub error: Option<String>,
        pub payload: Value,
    }

    #[derive(Debug, Serialize)]
    pub struct StorageTelemetryTroubleshootResponse {
        pub stats: StorageTelemetryStatsResponse,
        pub error_samples: Vec<StorageTelemetryErrorSample>,
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
            .route("/api/v1/storage/telemetry", post(post_storage_telemetry))
            .route(
                "/api/v1/storage/telemetry/stats",
                get(get_storage_telemetry_stats),
            )
            .route(
                "/api/v1/storage/telemetry/troubleshoot",
                get(get_storage_telemetry_troubleshoot),
            )
            .route("/api/v1/event", post(accept_event))
            .route("/api/v1/rules", post(create_rule).get(list_rules))
            .route("/api/v1/rules/reload", post(reload_rules))
            .with_state(state)
    }

    async fn post_storage_telemetry(
        State(state): State<Arc<AppState>>,
        Json(req): Json<StorageTelemetryRequest>,
    ) -> impl IntoResponse {
        let msg = Message::new(req.topic.clone(), req.payload.clone());
        let msg_id = msg.id.to_string();
        let _ = state.event_bus.publish(msg);

        let event = events::ActiveModel {
            id: Set(msg_id.clone()),
            topic: Set(req.topic),
            payload: Set(req.payload),
            timestamp: Set(chrono::Utc::now().timestamp_millis()),
        };
        if let Err(e) = event.insert(&state.db).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }

        (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "id": msg_id })),
        )
    }

    async fn get_storage_telemetry_stats(
        State(state): State<Arc<AppState>>,
        Query(q): Query<StorageTelemetryStatsQuery>,
    ) -> impl IntoResponse {
        let prefix = q.topic_prefix.unwrap_or_else(|| "storage/".to_string());
        let max_rows = q.max_rows.unwrap_or(2000).min(20000);

        let now_ms = chrono::Utc::now().timestamp_millis();
        let (since, until) = if let Some(window_secs) = q.window_secs {
            if window_secs == 0 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "window_secs must be > 0" })),
                );
            }

            let window_ms = match (window_secs as i64).checked_mul(1000) {
                Some(v) => v,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": "window_secs too large" })),
                    );
                }
            };
            (Some(now_ms.saturating_sub(window_ms)), Some(now_ms))
        } else {
            (q.since, q.until)
        };

        let mut stmt = events::Entity::find()
            .filter(events::Column::Topic.like(format!("{}%", prefix)))
            .order_by_desc(events::Column::Timestamp)
            .limit(max_rows);

        if let Some(since) = since {
            stmt = stmt.filter(events::Column::Timestamp.gte(since));
        }
        if let Some(until) = until {
            stmt = stmt.filter(events::Column::Timestamp.lte(until));
        }

        let rows = match stmt.all(&state.db).await {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                );
            }
        };

        let mut counts: std::collections::HashMap<(String, String), u64> =
            std::collections::HashMap::new();

        for r in &rows {
            let service = r
                .payload
                .get("service")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let key = (r.topic.clone(), service);
            let entry = counts.entry(key).or_insert(0);
            *entry = entry.saturating_add(1);
        }

        let mut items = counts
            .into_iter()
            .map(|((topic, service), count)| StorageTelemetryStatsItem { topic, service, count })
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.count.cmp(&a.count));

        let resp = StorageTelemetryStatsResponse {
            topic_prefix: prefix,
            since,
            until,
            rows_scanned: rows.len() as u64,
            items,
        };

        (StatusCode::OK, Json(serde_json::to_value(resp).unwrap()))
    }

    async fn get_storage_telemetry_troubleshoot(
        State(state): State<Arc<AppState>>,
        Query(q): Query<StorageTelemetryTroubleshootQuery>,
    ) -> impl IntoResponse {
        let prefix = q.topic_prefix.clone().unwrap_or_else(|| "storage/".to_string());
        let window_secs = q.window_secs.unwrap_or(300);
        let top = q.top.unwrap_or(20).min(200);
        let samples = q.samples.unwrap_or(10).min(200);

        let now_ms = chrono::Utc::now().timestamp_millis();
        let since_ms = now_ms.saturating_sub(window_secs as i64 * 1000);

        // 1) Recent write_err samples
        let rows = match events::Entity::find()
            .filter(events::Column::Topic.eq("storage/write_err"))
            .filter(events::Column::Timestamp.gte(since_ms))
            .order_by_desc(events::Column::Timestamp)
            .limit(samples)
            .all(&state.db)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                );
            }
        };

        let mut error_samples = Vec::new();
        for r in rows {
            let service = r
                .payload
                .get("service")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let stream_id = r
                .payload
                .get("stream_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let error = r
                .payload
                .get("error")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            error_samples.push(StorageTelemetryErrorSample {
                timestamp: r.timestamp,
                topic: r.topic,
                service,
                stream_id,
                error,
                payload: r.payload,
            });
        }

        // 2) Stats aggregation
        let rows_for_stats = match events::Entity::find()
            .filter(events::Column::Topic.like(format!("{}%", prefix)))
            .filter(events::Column::Timestamp.gte(since_ms))
            .filter(events::Column::Timestamp.lte(now_ms))
            .order_by_desc(events::Column::Timestamp)
            .limit(20000)
            .all(&state.db)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                );
            }
        };

        let mut counts: std::collections::HashMap<(String, String), u64> =
            std::collections::HashMap::new();
        for r in &rows_for_stats {
            let service = r
                .payload
                .get("service")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let key = (r.topic.clone(), service);
            let entry = counts.entry(key).or_insert(0);
            *entry = entry.saturating_add(1);
        }

        let mut items = counts
            .into_iter()
            .map(|((topic, service), count)| StorageTelemetryStatsItem { topic, service, count })
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.count.cmp(&a.count));
        items.truncate(top);

        let stats = StorageTelemetryStatsResponse {
            topic_prefix: prefix.clone(),
            since: Some(since_ms),
            until: Some(now_ms),
            rows_scanned: rows_for_stats.len() as u64,
            items,
        };

        let resp = StorageTelemetryTroubleshootResponse {
            stats,
            error_samples,
        };

        (
            StatusCode::OK,
            Json(serde_json::to_value(resp).unwrap()),
        )
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
