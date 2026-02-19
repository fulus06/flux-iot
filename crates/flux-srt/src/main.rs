mod receiver;
mod sender;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use flux_media_core::{
    snapshot::SnapshotOrchestrator,
    storage::filesystem::FileSystemStorage,
    types::StreamId,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tracing::info;

use receiver::SrtReceiver;

#[derive(Parser, Debug)]
#[command(author, version, about = "FLUX SRT Media Server")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:8085")]
    http_bind: String,

    #[arg(long, default_value = "./data/srt/storage")]
    storage_dir: String,

    #[arg(long, default_value = "./data/srt/keyframes")]
    keyframe_dir: String,
}

#[derive(Clone)]
struct AppState {
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
    streams: Arc<RwLock<HashMap<String, StreamInfo>>>,
}

#[derive(Debug, Clone)]
struct StreamInfo {
    stream_id: StreamId,
    port: u16,
    start_time: chrono::DateTime<chrono::Utc>,
    packet_count: u64,
}

async fn health() -> &'static str {
    "OK"
}

async fn list_streams(State(state): State<AppState>) -> impl IntoResponse {
    let streams = state.streams.read().await;
    let stream_list: Vec<serde_json::Value> = streams
        .values()
        .map(|info| {
            serde_json::json!({
                "stream_id": info.stream_id.as_str(),
                "port": info.port,
                "start_time": info.start_time.to_rfc3339(),
                "packet_count": info.packet_count,
            })
        })
        .collect();

    Json(serde_json::json!({ "streams": stream_list }))
}

#[derive(serde::Deserialize)]
struct StartStreamRequest {
    port: u16,
    stream_name: String,
}

async fn start_stream(
    State(state): State<AppState>,
    Json(req): Json<StartStreamRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let stream_id = StreamId::new("srt", &req.stream_name);
    let key = format!("{}:{}", req.port, req.stream_name);

    // 检查是否已存在
    {
        let streams = state.streams.read().await;
        if streams.contains_key(&key) {
            return Err(StatusCode::CONFLICT);
        }
    }

    // 注册流
    {
        let mut streams = state.streams.write().await;
        streams.insert(
            key.clone(),
            StreamInfo {
                stream_id: stream_id.clone(),
                port: req.port,
                start_time: chrono::Utc::now(),
                packet_count: 0,
            },
        );
    }

    // 启动 SRT 接收器
    let (receiver, mut rx) = SrtReceiver::new(req.port)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let streams_clone = state.streams.clone();
    tokio::spawn(async move {
        tokio::spawn(async move {
            receiver.start().await;
        });

        // 处理接收到的数据
        while let Some(packet) = rx.recv().await {
            if !packet.is_control {
                // 更新包计数
                let mut streams = streams_clone.write().await;
                if let Some(info) = streams.get_mut(&key) {
                    info.packet_count += 1;
                }
            }
        }
    });

    Ok(Json(serde_json::json!({
        "status": "started",
        "stream_id": stream_id.as_str(),
        "port": req.port,
    })))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    // 初始化存储
    let storage_config = flux_media_core::storage::StorageConfig {
        root_dir: PathBuf::from(&args.storage_dir),
        retention_days: 7,
        segment_duration_secs: 60,
    };
    let storage = Arc::new(RwLock::new(FileSystemStorage::new(storage_config)?));
    let orchestrator = Arc::new(SnapshotOrchestrator::new(PathBuf::from(
        &args.keyframe_dir,
    )));

    let state = AppState {
        storage,
        orchestrator,
        streams: Arc::new(RwLock::new(HashMap::new())),
    };

    info!(target: "srt", "SRT Media Server ready");

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/srt/streams", get(list_streams).post(start_stream))
        .with_state(state);

    let addr = args.http_bind;
    info!(target: "srt", "HTTP API listening on {}", addr);

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
