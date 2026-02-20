mod handshake;
mod packet;
mod receiver;
mod sender;
mod telemetry;

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
use flux_storage::{DiskType, PoolConfig, StorageManager};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tracing::info;

use telemetry::TelemetryClient;

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

    #[arg(long)]
    telemetry_endpoint: Option<String>,

    #[arg(long, default_value_t = 1000)]
    telemetry_timeout_ms: u64,
}

#[derive(Clone)]
struct AppState {
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
    streams: Arc<RwLock<HashMap<String, StreamInfo>>>,
    timeshift: Option<Arc<flux_media_core::timeshift::TimeShiftCore>>,
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
    let timeshift_clone = state.timeshift.clone();
    let stream_id_clone = stream_id.clone();
    
    tokio::spawn(async move {
        tokio::spawn(async move {
            receiver.start().await;
        });

        // 处理接收到的数据
        while let Some(packet) = rx.recv().await {
            if !packet.is_control {
                // 添加到时移
                if let Some(ref ts) = timeshift_clone {
                    use flux_media_core::timeshift::{Segment, SegmentFormat, SegmentMetadata};
                    use chrono::Utc;
                    
                    let segment = Segment {
                        sequence: packet.timestamp as u64,
                        start_time: Utc::now(),
                        duration: 0.04,
                        data: packet.data.clone(),
                        metadata: SegmentMetadata {
                            format: SegmentFormat::Raw,
                            has_keyframe: false,
                            file_path: None,
                            size: packet.data.len() as u64,
                        },
                    };
                    
                    let _ = ts.add_segment(&stream_id_clone.as_str(), segment).await;
                }
                
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

    // 加载配置
    use flux_config::ConfigLoader;
    use flux_media_core::timeshift::{TimeShiftCore, TimeShiftConfig};
    
    let config_loader = ConfigLoader::new("./config");
    let timeshift_config = config_loader.load_timeshift_config("srt")
        .unwrap_or_else(|_| {
            tracing::warn!("Failed to load SRT config, using defaults");
            flux_config::TimeShiftProtocolConfig::default()
                .merge_with_global(&flux_config::TimeShiftGlobalConfig::default())
        });

    // 初始化统一存储池（flux-storage）
    let storage_manager = Arc::new(StorageManager::new());
    let pool_configs = match config_loader.load_storage_pools("srt") {
        Ok(Some(pools)) => pools,
        Ok(None) => vec![PoolConfig {
            name: "default".to_string(),
            path: PathBuf::from(&args.storage_dir),
            disk_type: DiskType::Unknown,
            priority: 1,
            max_usage_percent: 95.0,
        }],
        Err(e) => {
            tracing::warn!(target: "srt", "Failed to load storage pools config, fallback to CLI storage_dir: {}", e);
            vec![PoolConfig {
                name: "default".to_string(),
                path: PathBuf::from(&args.storage_dir),
                disk_type: DiskType::Unknown,
                priority: 1,
                max_usage_percent: 95.0,
            }]
        }
    };
    if let Err(e) = storage_manager.initialize(pool_configs).await {
        tracing::warn!(target: "srt", "StorageManager initialize failed, fallback to local dir: {}", e);
    }
    let selected_root = storage_manager
        .select_pool(0)
        .await
        .unwrap_or_else(|_| PathBuf::from(&args.storage_dir))
        .join("srt");

    let telemetry = TelemetryClient::new(args.telemetry_endpoint.clone(), args.telemetry_timeout_ms);
    if telemetry.enabled() {
        telemetry
            .post(
                "storage/service_start",
                serde_json::json!({
                    "service": "flux-srt",
                    "selected_root": selected_root.to_string_lossy().to_string(),
                    "storage_dir": args.storage_dir,
                    "keyframe_dir": args.keyframe_dir,
                }),
            )
            .await;
    }

    // 初始化存储
    let storage_config = flux_media_core::storage::StorageConfig {
        root_dir: selected_root,
        retention_days: 7,
        segment_duration_secs: 60,
    };
    let storage = Arc::new(RwLock::new(FileSystemStorage::new(storage_config)?));
    let orchestrator = Arc::new(SnapshotOrchestrator::new(PathBuf::from(
        &args.keyframe_dir,
    )));

    // 创建时移核心
    let timeshift = if timeshift_config.enabled {
        let ts_config = TimeShiftConfig {
            enabled: timeshift_config.enabled,
            hot_cache_duration: timeshift_config.hot_cache_duration,
            cold_storage_duration: timeshift_config.cold_storage_duration,
            max_segments: timeshift_config.max_segments,
            batch_write_size: timeshift_config.batch_write_size,
            batch_write_interval: timeshift_config.batch_write_interval,
            lru_cache_size_mb: timeshift_config.lru_cache_size_mb,
        };
        Some(Arc::new(TimeShiftCore::new(
            ts_config,
            timeshift_config.storage_root.join("srt")
        )))
    } else {
        None
    };

    let state = AppState {
        storage,
        orchestrator,
        streams: Arc::new(RwLock::new(HashMap::new())),
        timeshift,
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
