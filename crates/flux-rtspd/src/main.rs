mod h264_depacketizer;
mod rtp_receiver;
mod rtsp_client;
mod sdp_parser;
mod stream_manager;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use flux_media_core::{
    snapshot::{SnapshotMode, SnapshotOrchestrator, SnapshotRequest},
    storage::{filesystem::FileSystemStorage, StorageConfig},
    types::StreamId,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about = "FLUX RTSP Media Server")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:554")]
    rtsp_bind: String,

    #[arg(long, default_value = "0.0.0.0:8083")]
    http_bind: String,

    #[arg(long, default_value = "./data/rtsp/storage")]
    storage_dir: String,

    #[arg(long, default_value = "./data/rtsp/keyframes")]
    keyframe_dir: String,
}

#[derive(Clone)]
struct AppState {
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
    stream_manager: Arc<stream_manager::RtspStreamManager>,
}


async fn health() -> &'static str {
    "OK"
}

async fn list_streams(State(state): State<AppState>) -> impl IntoResponse {
    let streams = state.stream_manager.list_streams().await;
    let stream_list: Vec<serde_json::Value> = streams
        .iter()
        .map(|info| {
            serde_json::json!({
                "stream_id": info.stream_id.as_str(),
                "url": info.url,
                "start_time": info.start_time.to_rfc3339(),
                "is_active": info.is_active,
                "frame_count": info.frame_count,
                "last_keyframe_time": info.last_keyframe_time.map(|t| t.to_rfc3339()),
            })
        })
        .collect();

    Json(serde_json::json!({ "streams": stream_list }))
}

async fn snapshot(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> std::result::Result<Response, StatusCode> {
    let media_stream_id = StreamId::from_string(stream_id);
    let req = SnapshotRequest {
        stream_id: media_stream_id,
        mode: SnapshotMode::Auto,
        width: None,
        height: None,
    };

    let snapshot = state
        .orchestrator
        .get_snapshot(req)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let mut resp: Response = snapshot.data.into_response();
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/octet-stream"),
    );
    Ok(resp)
}

#[derive(serde::Deserialize)]
struct StartStreamRequest {
    url: String,
}

async fn start_stream(
    State(state): State<AppState>,
    Json(req): Json<StartStreamRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    state.stream_manager
        .start_stream(req.url.clone())
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    Ok(Json(serde_json::json!({
        "status": "started",
        "url": req.url
    })))
}

#[derive(serde::Deserialize)]
struct StopStreamRequest {
    url: String,
}

async fn stop_stream(
    State(state): State<AppState>,
    Json(req): Json<StopStreamRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    state.stream_manager
        .stop_stream(&req.url)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    
    Ok(Json(serde_json::json!({
        "status": "stopped",
        "url": req.url
    })))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    // 初始化存储
    let storage_config = StorageConfig {
        root_dir: PathBuf::from(&args.storage_dir),
        retention_days: 7,
        segment_duration_secs: 60,
    };
    let storage = Arc::new(RwLock::new(FileSystemStorage::new(storage_config)?));
    let orchestrator = Arc::new(SnapshotOrchestrator::new(PathBuf::from(
        &args.keyframe_dir,
    )));

    // 创建流管理器
    let stream_manager = Arc::new(stream_manager::RtspStreamManager::new(
        storage.clone(),
        orchestrator.clone(),
    ));

    let state = AppState {
        storage,
        orchestrator,
        stream_manager: stream_manager.clone(),
    };

    // RTSP 流管理器已就绪
    tracing::info!(target: "rtspd", "RTSP stream manager ready");
    tracing::info!(target: "rtspd", "Use POST /api/v1/rtsp/streams to start streams");

    // 启动 HTTP API 服务器
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/rtsp/streams", get(list_streams).post(start_stream))
        .route("/api/v1/rtsp/streams/stop", post(stop_stream))
        .route("/api/v1/rtsp/streams/:stream_id/snapshot", get(snapshot))
        .with_state(state);

    let addr = args.http_bind;
    tracing::info!(target: "rtspd", "HTTP API listening on {}", addr);

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

    #[test]
    fn test_stream_id_format() {
        let stream_id = StreamId::new("rtsp", "192.168.1.100/stream1");
        assert_eq!(stream_id.as_str(), "rtsp/192.168.1.100/stream1");
        assert_eq!(stream_id.protocol(), Some("rtsp"));
    }
}
