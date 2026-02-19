mod client;
mod discovery;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::info;

use client::{OnvifClient, OnvifProfile};
use discovery::{OnvifDevice, OnvifDiscovery};

#[derive(Parser, Debug)]
#[command(author, version, about = "FLUX ONVIF Device Manager")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:8084")]
    http_bind: String,
}

#[derive(Clone)]
struct AppState {
    devices: Arc<RwLock<HashMap<String, OnvifDevice>>>,
    rtsp_urls: Arc<RwLock<HashMap<String, String>>>,
}

async fn health() -> &'static str {
    "OK"
}

async fn discover_devices(State(state): State<AppState>) -> impl IntoResponse {
    let discovery = match OnvifDiscovery::new() {
        Ok(d) => d,
        Err(e) => return Json(serde_json::json!({
            "error": format!("Failed to create discovery: {}", e)
        })),
    };

    let devices = match discovery.discover() {
        Ok(d) => d,
        Err(e) => return Json(serde_json::json!({
            "error": format!("Discovery failed: {}", e)
        })),
    };

    // 保存设备
    {
        let mut devices_lock = state.devices.write().await;
        for device in &devices {
            devices_lock.insert(device.uuid.clone(), device.clone());
        }
    }

    let device_list: Vec<serde_json::Value> = devices
        .iter()
        .map(|d| {
            serde_json::json!({
                "uuid": d.uuid,
                "name": d.name,
                "hardware": d.hardware,
                "location": d.location,
                "service_url": d.service_url,
            })
        })
        .collect();

    Json(serde_json::json!({
        "devices": device_list,
        "count": devices.len()
    }))
}

async fn list_devices(State(state): State<AppState>) -> impl IntoResponse {
    let devices = state.devices.read().await;
    let device_list: Vec<serde_json::Value> = devices
        .values()
        .map(|d| {
            serde_json::json!({
                "uuid": d.uuid,
                "name": d.name,
                "hardware": d.hardware,
                "location": d.location,
                "service_url": d.service_url,
            })
        })
        .collect();

    Json(serde_json::json!({ "devices": device_list }))
}

#[derive(serde::Deserialize)]
struct GetStreamUriRequest {
    uuid: String,
    username: Option<String>,
    password: Option<String>,
}

async fn get_stream_uri(
    State(state): State<AppState>,
    Json(req): Json<GetStreamUriRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    // 获取设备
    let device = {
        let devices = state.devices.read().await;
        devices
            .get(&req.uuid)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)?
    };

    // 创建 ONVIF 客户端
    let mut client = OnvifClient::new(device.service_url.clone());
    if let (Some(username), Some(password)) = (req.username, req.password) {
        client = client.with_auth(username, password);
    }

    // 获取 Profiles
    let profiles = client
        .get_profiles()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if profiles.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    // 获取第一个 Profile 的流 URI
    let media_uri = client
        .get_stream_uri(&profiles[0].token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 保存 RTSP URL
    {
        let mut rtsp_urls = state.rtsp_urls.write().await;
        rtsp_urls.insert(req.uuid.clone(), media_uri.uri.clone());
    }

    Ok(Json(serde_json::json!({
        "uuid": req.uuid,
        "rtsp_url": media_uri.uri,
        "profile_token": media_uri.profile_token,
        "profile_name": profiles[0].name,
    })))
}

async fn get_device_info(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let device = {
        let devices = state.devices.read().await;
        devices.get(&uuid).cloned().ok_or(StatusCode::NOT_FOUND)?
    };

    let client = OnvifClient::new(device.service_url.clone());
    let info = client
        .get_device_information()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "uuid": uuid,
        "manufacturer": info.manufacturer,
        "model": info.model,
        "firmware_version": info.firmware_version,
        "serial_number": info.serial_number,
        "hardware_id": info.hardware_id,
    })))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    let state = AppState {
        devices: Arc::new(RwLock::new(HashMap::new())),
        rtsp_urls: Arc::new(RwLock::new(HashMap::new())),
    };

    info!(target: "onvif", "ONVIF Device Manager ready");

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/onvif/discover", post(discover_devices))
        .route("/api/v1/onvif/devices", get(list_devices))
        .route("/api/v1/onvif/devices/:uuid/info", get(get_device_info))
        .route("/api/v1/onvif/stream_uri", post(get_stream_uri))
        .with_state(state);

    let addr = args.http_bind;
    info!(target: "onvif", "HTTP API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let response = health().await;
        assert_eq!(response, "OK");
    }
}
