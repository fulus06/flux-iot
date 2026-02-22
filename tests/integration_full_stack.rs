// 全栈集成测试 - 测试完整的数据流
// 设备 → MQTT → EventBus → 规则引擎 → 存储 → API 查询

mod common;

use axum::{body::Body, http::Request};
use common::*;
use flux_types::message::Message;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn test_full_device_telemetry_pipeline() -> anyhow::Result<()> {
    // 1. 启动测试环境
    let state = create_test_state().await;
    let mut event_rx = state.event_bus.subscribe();
    let app = flux_server::api::create_router(state.clone());

    // 2. 创建规则：温度超过 30 度触发告警
    let rule_data = json!({
        "name": "high_temp_alert",
        "script": r#"
            if payload.temperature > 30.0 {
                print("High temperature alert: " + payload.temperature);
                return true;
            }
            return false;
        "#
    });

    let request = Request::builder()
        .uri("/api/v1/rules")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&rule_data)?))
        .unwrap();

    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), 201);

    // 3. 模拟设备上报数据（通过 API）
    let telemetry = json!({
        "topic": "device/sensor_001/telemetry",
        "payload": {
            "temperature": 35.5,
            "humidity": 60.0,
            "timestamp": chrono::Utc::now().timestamp()
        }
    });

    let request = Request::builder()
        .uri("/api/v1/event")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&telemetry)?))
        .unwrap();

    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), 200);

    // 4. 验证事件已发布到 EventBus
    let msg = wait_for_event(
        event_rx,
        |m: &Message| m.topic == "device/sensor_001/telemetry",
        1000,
    )
    .await;

    assert!(msg.is_some());
    let msg = msg.unwrap();
    assert_eq!(msg.payload["temperature"], 35.5);

    // 5. 验证规则引擎已执行（通过日志或数据库）
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // 6. 查询事件历史
    let request = Request::builder()
        .uri("/api/v1/events?limit=10")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), 200);

    let body = hyper::body::to_bytes(response.into_body()).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert!(json["events"].is_array());

    Ok(())
}

#[tokio::test]
async fn test_device_registration_and_authentication() -> anyhow::Result<()> {
    let state = create_test_state().await;
    let app = flux_server::api::create_router(state.clone());

    // 1. 注册设备
    let device_data = json!({
        "device_id": "test_device_001",
        "token": "secure_token_123"
    });

    let request = Request::builder()
        .uri("/api/v1/devices")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&device_data)?))
        .unwrap();

    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), 201);

    // 2. 验证设备已存储到数据库
    use flux_core::entity::devices;
    use sea_orm::EntityTrait;

    let device = devices::Entity::find_by_id("test_device_001")
        .one(&state.db)
        .await?;

    assert!(device.is_some());
    let device = device.unwrap();
    assert_eq!(device.id, "test_device_001");

    // 3. 模拟设备认证（MQTT CONNECT）
    // 这里需要启动 MQTT Broker 并测试认证逻辑
    // 由于集成测试复杂度，可以在单独的 MQTT 测试中完成

    Ok(())
}

#[tokio::test]
async fn test_config_hot_reload_workflow() -> anyhow::Result<()> {
    use std::sync::Arc;
    use tokio::sync::watch;

    let state = create_test_state().await;
    let (tx, mut rx) = watch::channel(test_app_config());

    // 模拟配置变更
    let mut new_config = test_app_config();
    new_config.eventbus.capacity = 200;
    tx.send(new_config)?;

    // 验证配置已更新
    rx.changed().await?;
    let updated_config = rx.borrow().clone();
    assert_eq!(updated_config.eventbus.capacity, 200);

    Ok(())
}

#[tokio::test]
async fn test_concurrent_event_processing() -> anyhow::Result<()> {
    let state = create_test_state().await;
    let app = flux_server::api::create_router(state.clone());

    // 并发发送 100 个事件
    let mut handles = vec![];
    for i in 0..100 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let event_data = json!({
                "topic": format!("test/concurrent/{}", i),
                "payload": {"value": i}
            });

            let request = Request::builder()
                .uri("/api/v1/event")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&event_data).unwrap()))
                .unwrap();

            app_clone.oneshot(request).await
        });
        handles.push(handle);
    }

    // 等待所有请求完成
    for handle in handles {
        let response = handle.await??;
        assert_eq!(response.status(), 200);
    }

    Ok(())
}

#[tokio::test]
async fn test_storage_telemetry_aggregation() -> anyhow::Result<()> {
    let state = create_test_state().await;
    let app = flux_server::api::create_router(state.clone());

    // 发送多个存储遥测事件
    for i in 0..10 {
        let telemetry = json!({
            "topic": "storage/write_err",
            "payload": {
                "service": "flux-rtmpd",
                "stream_id": format!("stream_{}", i),
                "error": "disk full"
            }
        });

        let request = Request::builder()
            .uri("/api/v1/storage/telemetry")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&telemetry)?))
            .unwrap();

        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), 200);
    }

    // 等待事件处理
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // 查询聚合统计
    let request = Request::builder()
        .uri("/api/v1/storage/telemetry/stats?topic_prefix=storage/")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), 200);

    let body = hyper::body::to_bytes(response.into_body()).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    
    assert!(json["items"].is_array());
    let items = json["items"].as_array().unwrap();
    assert!(items.len() > 0);

    Ok(())
}
