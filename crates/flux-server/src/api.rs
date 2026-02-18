use crate::{metrics, AppState};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use config::{Config, File, FileFormat};
use flux_types::message::Message;
use serde::Deserialize;
use serde_json::Value;
use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait, Value as SeaValue};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct EventRequest {
    pub topic: String,
    pub payload: Value,
}

fn gb_device_to_json(device: &flux_video::gb28181::sip::Device) -> serde_json::Value {
    let status = match device.status {
        flux_video::gb28181::sip::DeviceStatus::Online => "online",
        flux_video::gb28181::sip::DeviceStatus::Offline => "offline",
        flux_video::gb28181::sip::DeviceStatus::Registering => "registering",
    };

    serde_json::json!({
        "device_id": device.device_id,
        "name": device.name,
        "ip": device.ip,
        "port": device.port,
        "status": status,
        "register_time_ms": device.register_time.timestamp_millis(),
        "last_keepalive_ms": device.last_keepalive.timestamp_millis(),
        "expires": device.expires,
        "transport": device.transport,
        "manufacturer": device.manufacturer,
        "model": device.model,
        "firmware": device.firmware,
    })
}

fn gb_channel_to_json(channel: &flux_video::gb28181::sip::Channel) -> serde_json::Value {
    serde_json::json!({
        "channel_id": channel.channel_id,
        "name": channel.name,
        "manufacturer": channel.manufacturer,
        "model": channel.model,
        "status": channel.status,
        "parent_id": channel.parent_id,
        "longitude": channel.longitude,
        "latitude": channel.latitude,
    })
}

pub async fn get_app_config_audit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_admin_auth(&headers) {
        return e;
    }

    let Some(db) = state.config_db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "config_db is not configured; run with --config-source sqlite|postgres"
            })),
        );
    };

    if let Err(e) = ensure_app_config_audit_table(db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let backend = db.get_database_backend();
    let stmt = Statement::from_string(
        backend,
        "SELECT id, prev_updated_at, new_updated_at, prev_hash, new_hash, user_agent, forwarded_for, created_at \
         FROM app_config_audit ORDER BY created_at DESC LIMIT 50"
            .to_string(),
    );

    let rows = match db.query_all(stmt).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let mut items: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = match row.try_get("", "id") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let prev_updated_at: Option<i64> = match row.try_get("", "prev_updated_at") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let new_updated_at: i64 = match row.try_get("", "new_updated_at") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let prev_hash: Option<String> = match row.try_get("", "prev_hash") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let new_hash: String = match row.try_get("", "new_hash") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let user_agent: Option<String> = match row.try_get("", "user_agent") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let forwarded_for: Option<String> = match row.try_get("", "forwarded_for") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let created_at: i64 = match row.try_get("", "created_at") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };

        items.push(serde_json::json!({
            "id": id,
            "prev_updated_at": prev_updated_at,
            "new_updated_at": new_updated_at,
            "prev_hash": prev_hash,
            "new_hash": new_hash,
            "user_agent": user_agent,
            "forwarded_for": forwarded_for,
            "created_at": created_at,
        }));
    }

    (StatusCode::OK, Json(serde_json::json!({ "items": items })))
}

fn check_reload_policy(old_cfg: &flux_server::AppConfig, new_cfg: &flux_server::AppConfig) -> Option<Vec<&'static str>> {
    let mut blocked: Vec<&'static str> = Vec::new();

    if old_cfg.server.host != new_cfg.server.host {
        blocked.push("server.host");
    }
    if old_cfg.server.port != new_cfg.server.port {
        blocked.push("server.port");
    }
    if old_cfg.database.url != new_cfg.database.url {
        blocked.push("database.url");
    }
    if old_cfg.plugins.directory != new_cfg.plugins.directory {
        blocked.push("plugins.directory");
    }

    // GB28181 SIP 绑定/标识类配置涉及 socket 绑定和协议身份，不允许热更新。
    if old_cfg.gb28181.enabled != new_cfg.gb28181.enabled {
        blocked.push("gb28181.enabled");
    }
    if old_cfg.gb28181.sip.bind_addr != new_cfg.gb28181.sip.bind_addr {
        blocked.push("gb28181.sip.bind_addr");
    }
    if old_cfg.gb28181.sip.sip_domain != new_cfg.gb28181.sip.sip_domain {
        blocked.push("gb28181.sip.sip_domain");
    }
    if old_cfg.gb28181.sip.sip_id != new_cfg.gb28181.sip.sip_id {
        blocked.push("gb28181.sip.sip_id");
    }
    if old_cfg.gb28181.sip.device_expires != new_cfg.gb28181.sip.device_expires {
        blocked.push("gb28181.sip.device_expires");
    }
    if old_cfg.gb28181.sip.session_timeout != new_cfg.gb28181.sip.session_timeout {
        blocked.push("gb28181.sip.session_timeout");
    }

    if blocked.is_empty() {
        None
    } else {
        Some(blocked)
    }
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub script: String,
}

#[derive(Deserialize)]
pub struct UpdateAppConfigRequest {
    pub content: String,
}

#[derive(Deserialize)]
pub struct GbInviteRequest {
    pub device_id: String,
    pub channel_id: String,
    pub rtp_port: u16,
}

#[derive(Deserialize)]
pub struct GbByeRequest {
    pub call_id: String,
}

#[derive(Deserialize)]
pub struct GbDeviceRequest {
    pub device_id: String,
}

fn gb_unavailable() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": "gb28181 sip service is not enabled" })),
    )
}

fn map_video_error(e: flux_video::VideoError) -> (StatusCode, Json<serde_json::Value>) {
    let msg = e.to_string();
    let status = if msg.to_ascii_lowercase().contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(serde_json::json!({ "error": msg })))
}

pub async fn gb_invite(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbInviteRequest>,
) -> impl IntoResponse {
    let Some(sip) = state.gb28181_sip.as_ref() else {
        return gb_unavailable();
    };

    match sip
        .start_realtime_play(&req.device_id, &req.channel_id, req.rtp_port)
        .await
    {
        Ok(call_id) => (
            StatusCode::OK,
            Json(serde_json::json!({ "call_id": call_id })),
        ),
        Err(e) => map_video_error(e),
    }
}

pub async fn gb_bye(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbByeRequest>,
) -> impl IntoResponse {
    let Some(sip) = state.gb28181_sip.as_ref() else {
        return gb_unavailable();
    };

    match sip.stop_realtime_play(&req.call_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => map_video_error(e),
    }
}

pub async fn gb_query_catalog(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbDeviceRequest>,
) -> impl IntoResponse {
    let Some(sip) = state.gb28181_sip.as_ref() else {
        return gb_unavailable();
    };

    match sip.query_catalog(&req.device_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => map_video_error(e),
    }
}

pub async fn gb_query_device_info(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbDeviceRequest>,
) -> impl IntoResponse {
    let Some(sip) = state.gb28181_sip.as_ref() else {
        return gb_unavailable();
    };

    match sip.query_device_info(&req.device_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => map_video_error(e),
    }
}

pub async fn gb_query_device_status(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbDeviceRequest>,
) -> impl IntoResponse {
    let Some(sip) = state.gb28181_sip.as_ref() else {
        return gb_unavailable();
    };

    match sip.query_device_status(&req.device_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => map_video_error(e),
    }
}

pub async fn gb_list_devices(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let Some(sip) = state.gb28181_sip.as_ref() else {
        return gb_unavailable();
    };

    let devices = sip.device_manager().list_devices().await;
    let devices: Vec<serde_json::Value> = devices.iter().map(gb_device_to_json).collect();
    (StatusCode::OK, Json(serde_json::json!({ "devices": devices })))
}

pub async fn gb_get_device(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let Some(sip) = state.gb28181_sip.as_ref() else {
        return gb_unavailable();
    };

    match sip.device_manager().get_device(&device_id).await {
        Some(device) => (
            StatusCode::OK,
            Json(serde_json::json!({ "device": gb_device_to_json(&device) })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "device not found" })),
        ),
    }
}

pub async fn gb_list_device_channels(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let Some(sip) = state.gb28181_sip.as_ref() else {
        return gb_unavailable();
    };

    match sip.device_manager().get_device(&device_id).await {
        Some(device) => {
            let channels: Vec<serde_json::Value> =
                device.channels.iter().map(gb_channel_to_json).collect();
            (StatusCode::OK, Json(serde_json::json!({ "channels": channels })))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "device not found" })),
        ),
    }
}

fn require_admin_auth(headers: &HeaderMap) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let token = match std::env::var("FLUX_ADMIN_TOKEN") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "FLUX_ADMIN_TOKEN is not configured"
                })),
            ))
        }
    };

    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let expected = format!("Bearer {}", token);
    if auth != expected {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized" })),
        ));
    }

    Ok(())
}

async fn ensure_app_config_audit_table(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    let backend = db.get_database_backend();

    let sql = match backend {
        DbBackend::Sqlite => {
            "CREATE TABLE IF NOT EXISTS app_config_audit (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                prev_updated_at INTEGER,\
                new_updated_at INTEGER NOT NULL,\
                prev_hash TEXT,\
                new_hash TEXT NOT NULL,\
                user_agent TEXT,\
                forwarded_for TEXT,\
                created_at INTEGER NOT NULL\
            )"
        }
        DbBackend::Postgres => {
            "CREATE TABLE IF NOT EXISTS app_config_audit (\
                id BIGSERIAL PRIMARY KEY,\
                prev_updated_at BIGINT,\
                new_updated_at BIGINT NOT NULL,\
                prev_hash TEXT,\
                new_hash TEXT NOT NULL,\
                user_agent TEXT,\
                forwarded_for TEXT,\
                created_at BIGINT NOT NULL\
            )"
        }
        DbBackend::MySql => {
            "CREATE TABLE IF NOT EXISTS app_config_audit (\
                id BIGINT AUTO_INCREMENT PRIMARY KEY,\
                prev_updated_at BIGINT,\
                new_updated_at BIGINT NOT NULL,\
                prev_hash TEXT,\
                new_hash TEXT NOT NULL,\
                user_agent TEXT,\
                forwarded_for TEXT,\
                created_at BIGINT NOT NULL\
            )"
        }
    };

    db.execute(Statement::from_string(backend, sql.to_string()))
        .await?;
    Ok(())
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

async fn ensure_app_config_table(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    let backend = db.get_database_backend();

    let sql = match backend {
        DbBackend::Sqlite => {
            "CREATE TABLE IF NOT EXISTS app_config (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                content TEXT NOT NULL,\
                updated_at INTEGER NOT NULL\
            )"
        }
        DbBackend::Postgres => {
            "CREATE TABLE IF NOT EXISTS app_config (\
                id BIGSERIAL PRIMARY KEY,\
                content TEXT NOT NULL,\
                updated_at BIGINT NOT NULL\
            )"
        }
        DbBackend::MySql => {
            "CREATE TABLE IF NOT EXISTS app_config (\
                id BIGINT AUTO_INCREMENT PRIMARY KEY,\
                content TEXT NOT NULL,\
                updated_at BIGINT NOT NULL\
            )"
        }
    };

    db.execute(Statement::from_string(backend, sql.to_string()))
        .await?;
    Ok(())
}

pub async fn get_app_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_admin_auth(&headers) {
        return e;
    }

    let Some(db) = state.config_db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "config_db is not configured; run with --config-source sqlite|postgres"
            })),
        );
    };

    if let Err(e) = ensure_app_config_table(db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let backend = db.get_database_backend();
    let stmt = Statement::from_string(
        backend,
        "SELECT content, updated_at FROM app_config ORDER BY updated_at DESC LIMIT 1".to_string(),
    );

    let row_opt = match db.query_one(stmt).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let Some(row) = row_opt else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "no app_config found" })),
        );
    };

    let content: String = match row.try_get("", "content") {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };
    let updated_at: i64 = match row.try_get("", "updated_at") {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({ "content": content, "updated_at": updated_at })),
    )
}

pub async fn update_app_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<UpdateAppConfigRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_admin_auth(&headers) {
        return e;
    }

    let Some(db) = state.config_db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "config_db is not configured; run with --config-source sqlite|postgres"
            })),
        );
    };

    if let Err(e) = ensure_app_config_table(db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    // 先校验 TOML 语法 & 能反序列化为 AppConfig
    let settings = match Config::builder()
        .add_source(File::from_str(&req.content, FileFormat::Toml))
        .build()
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let new_cfg: flux_server::AppConfig = match settings.try_deserialize() {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let old_cfg = state.config.borrow().clone();
    if let Some(blocked) = check_reload_policy(&old_cfg, &new_cfg) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "config contains changes that require restart",
                "blocked": blocked,
            })),
        );
    }

    if let Err(e) = ensure_app_config_audit_table(db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let backend = db.get_database_backend();
    let now = chrono::Utc::now().timestamp_millis();

    let prev_stmt = Statement::from_string(
        backend,
        "SELECT content, updated_at FROM app_config ORDER BY updated_at DESC LIMIT 1".to_string(),
    );

    let prev_row_opt = match db.query_one(prev_stmt).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let (prev_updated_at, prev_hash) = if let Some(row) = prev_row_opt {
        let prev_content: String = match row.try_get("", "content") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let prev_updated_at: i64 = match row.try_get("", "updated_at") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };

        (Some(prev_updated_at), Some(sha256_hex(&prev_content)))
    } else {
        (None, None)
    };

    let new_hash = sha256_hex(&req.content);
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let forwarded_for = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let txn = match db.begin().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let insert_config_sql = match backend {
        DbBackend::Postgres => "INSERT INTO app_config (content, updated_at) VALUES ($1, $2)",
        DbBackend::Sqlite | DbBackend::MySql => {
            "INSERT INTO app_config (content, updated_at) VALUES (?, ?)"
        }
    };
    let stmt = Statement::from_sql_and_values(
        backend,
        insert_config_sql,
        vec![
            SeaValue::String(Some(Box::new(req.content))),
            SeaValue::BigInt(Some(now)),
        ],
    );

    if let Err(e) = txn.execute(stmt).await {
        let _ = txn.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let insert_audit_sql = match backend {
        DbBackend::Postgres => {
            "INSERT INTO app_config_audit (prev_updated_at, new_updated_at, prev_hash, new_hash, user_agent, forwarded_for, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)"
        }
        DbBackend::Sqlite | DbBackend::MySql => {
            "INSERT INTO app_config_audit (prev_updated_at, new_updated_at, prev_hash, new_hash, user_agent, forwarded_for, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        }
    };

    let stmt = Statement::from_sql_and_values(
        backend,
        insert_audit_sql,
        vec![
            SeaValue::BigInt(prev_updated_at),
            SeaValue::BigInt(Some(now)),
            SeaValue::String(prev_hash.map(Box::new)),
            SeaValue::String(Some(Box::new(new_hash.clone()))),
            SeaValue::String(user_agent.map(Box::new)),
            SeaValue::String(forwarded_for.map(Box::new)),
            SeaValue::BigInt(Some(now)),
        ],
    );

    if let Err(e) = txn.execute(stmt).await {
        let _ = txn.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    if let Err(e) = txn.commit().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "updated", "updated_at": now, "hash": new_hash })),
    )
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
        .route("/api/v1/gb28181/invite", post(gb_invite))
        .route("/api/v1/gb28181/bye", post(gb_bye))
        .route("/api/v1/gb28181/catalog", post(gb_query_catalog))
        .route("/api/v1/gb28181/device-info", post(gb_query_device_info))
        .route("/api/v1/gb28181/device-status", post(gb_query_device_status))
        .route("/api/v1/gb28181/devices", get(gb_list_devices))
        .route("/api/v1/gb28181/devices/:device_id", get(gb_get_device))
        .route(
            "/api/v1/gb28181/devices/:device_id/channels",
            get(gb_list_device_channels),
        )
        .route(
            "/api/v1/app-config",
            get(get_app_config).post(update_app_config),
        )
        .route("/api/v1/app-config/audit", get(get_app_config_audit))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use flux_core::bus::EventBus;
    use flux_plugin::manager::PluginManager;
    use flux_script::ScriptEngine;
    use sea_orm::Database;
    use std::sync::Once;
    use tokio::sync::watch;
    use tower::ServiceExt;

    static INIT: Once = Once::new();

    fn init_admin_token() {
        INIT.call_once(|| {
            std::env::set_var("FLUX_ADMIN_TOKEN", "test-token");
        });
    }

    async fn create_test_state_with_config_db() -> Arc<AppState> {
        let event_bus = Arc::new(EventBus::new(100));
        let plugin_manager = Arc::new(PluginManager::new().expect("plugin manager"));
        let script_engine = Arc::new(ScriptEngine::new());
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("db");
        let config_db = Database::connect("sqlite::memory:")
            .await
            .expect("config db");

        let (_tx, rx) = watch::channel(flux_server::AppConfig::default());

        Arc::new(AppState {
            event_bus,
            plugin_manager,
            script_engine,
            db,
            config_db: Some(config_db),
            config: rx,
            gb28181_sip: None,
        })
    }

    fn minimal_app_config_toml() -> String {
        r#"
[server]
host = "127.0.0.1"
port = 3000

[database]
url = "sqlite::memory:"

[plugins]
directory = "plugins"
"#
        .to_string()
    }

    fn minimal_app_config_toml_with_port(port: u16) -> String {
        format!(
            r#"
[server]
host = "127.0.0.1"
port = {port}

[database]
url = "sqlite::memory:"

[plugins]
directory = "plugins"
"#
        )
    }

    #[tokio::test]
    async fn test_app_config_unauthorized() {
        init_admin_token();
        let state = create_test_state_with_config_db().await;
        let app = create_router(state);

        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("GET")
            .body(Body::empty())
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_app_config_authorized_roundtrip() {
        init_admin_token();
        let state = create_test_state_with_config_db().await;
        let app = create_router(state);

        let body = serde_json::json!({ "content": minimal_app_config_toml() });
        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", "Bearer test-token")
            .body(Body::from(body.to_string()))
            .expect("request");

        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("GET")
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .expect("request");

        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .uri("/api/v1/app-config/audit")
            .method("GET")
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .expect("request");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_app_config_reject_blocked_fields() {
        init_admin_token();
        let state = create_test_state_with_config_db().await;
        let app = create_router(state);

        let body = serde_json::json!({ "content": minimal_app_config_toml() });
        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", "Bearer test-token")
            .body(Body::from(body.to_string()))
            .expect("request");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        // 修改 server.port（禁止热更新）应被拒绝
        let body = serde_json::json!({ "content": minimal_app_config_toml_with_port(3001) });
        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", "Bearer test-token")
            .body(Body::from(body.to_string()))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
