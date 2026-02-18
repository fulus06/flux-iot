use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use flux_core::bus::EventBus;
use flux_plugin::manager::PluginManager;
use flux_script::ScriptEngine;
use flux_server::{config::AppConfig, AppState};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Schema};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::watch;
use tower::ServiceExt;

async fn create_test_db() -> DatabaseConnection {
    Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database")
}

async fn create_test_state() -> Arc<AppState> {
    let event_bus = Arc::new(EventBus::new(100));
    let plugin_manager = Arc::new(PluginManager::new().unwrap());
    let script_engine = Arc::new(ScriptEngine::new());
    let db = create_test_db().await;

    // 创建表结构
    use flux_core::entity::rules;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(rules::Entity);
    let builder = db.get_database_backend();
    let _result = db
        .execute(builder.build(&stmt))
        .await
        .expect("Failed to create table");

    let (_tx, rx) = watch::channel(AppConfig::default());

    Arc::new(AppState {
        event_bus,
        plugin_manager,
        script_engine,
        db,
        config: rx,
    })
}

#[tokio::test]
async fn test_health_endpoint() {
    let state = create_test_state().await;
    let app = flux_server::api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_accept_event() {
    let state = create_test_state().await;
    let mut rx = state.event_bus.subscribe();
    let app = flux_server::api::create_router(state);

    let event_data = json!({
        "topic": "test/sensor",
        "payload": {"temperature": 25.5}
    });

    let request = Request::builder()
        .uri("/api/v1/event")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&event_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 验证事件是否发布到 EventBus
    let msg = tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.recv())
        .await
        .expect("Timeout")
        .expect("Failed to receive");

    assert_eq!(msg.topic, "test/sensor");
    assert_eq!(msg.payload["temperature"], 25.5);
}

#[tokio::test]
async fn test_list_rules_empty() {
    let state = create_test_state().await;
    let app = flux_server::api::create_router(state);

    let request = Request::builder()
        .uri("/api/v1/rules")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["rules"].is_array());
    assert_eq!(json["rules"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_create_rule_success() {
    let state = create_test_state().await;
    let app = flux_server::api::create_router(state.clone());

    let rule_data = json!({
        "name": "test_rule",
        "script": "if payload.temp > 30.0 { return true; } else { return false; }"
    });

    let request = Request::builder()
        .uri("/api/v1/rules")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&rule_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // 验证规则是否被编译
    let scripts = state.script_engine.get_script_ids();
    assert!(scripts.contains(&"test_rule".to_string()));
}

#[tokio::test]
async fn test_create_rule_invalid_script() {
    let state = create_test_state().await;
    let app = flux_server::api::create_router(state);

    let rule_data = json!({
        "name": "invalid_rule",
        "script": "this is not valid rhai syntax {{{"
    });

    let request = Request::builder()
        .uri("/api/v1/rules")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&rule_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_event_with_invalid_json() {
    let state = create_test_state().await;
    let app = flux_server::api::create_router(state);

    let request = Request::builder()
        .uri("/api/v1/event")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from("invalid json"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // Axum 会返回 400 或 422 对于无效的 JSON
    assert!(response.status().is_client_error());
}
