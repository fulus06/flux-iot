mod hls_manager;
mod media_processor;
mod rtmp_server;
mod stream_manager;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use tracing::{error, info};
use clap::Parser;
use flux_media_core::{
    playback::{FlvMuxer, FlvTag, HlsGenerator},
    snapshot::{SnapshotMode, SnapshotOrchestrator, SnapshotRequest},
    storage::{filesystem::FileSystemStorage, StorageConfig},
    types::StreamId,
};
use rtmp_server::RtmpServer;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[command(author, version, about = "FLUX RTMP Media Server")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:1935")]
    rtmp_bind: String,

    #[arg(long, default_value = "0.0.0.0:8082")]
    http_bind: String,

    #[arg(long, default_value = "./data/rtmp/storage")]
    storage_dir: String,

    #[arg(long, default_value = "./data/rtmp/keyframes")]
    keyframe_dir: String,
}

#[derive(Clone)]
struct AppState {
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
    streams: Arc<RwLock<HashMap<String, StreamInfo>>>,
    hls_generators: Arc<RwLock<HashMap<String, Arc<HlsGenerator>>>>,
    rtmp_server: Option<Arc<RtmpServer>>,
    hls_manager: Arc<hls_manager::HlsManager>,
    stream_manager: Arc<stream_manager::StreamManager>,
}

#[derive(Debug, Clone)]
struct StreamInfo {
    stream_id: StreamId,
    app_name: String,
    stream_key: String,
    start_time: chrono::DateTime<chrono::Utc>,
    video_frames: u64,
    audio_frames: u64,
}

async fn health() -> &'static str {
    "OK"
}

async fn list_streams(State(state): State<AppState>) -> impl IntoResponse {
    let stream_list: Vec<serde_json::Value> = if let Some(rtmp_server) = &state.rtmp_server {
        rtmp_server
            .get_active_streams()
            .await
            .iter()
            .map(|info| {
                serde_json::json!({
                    "stream_id": info.stream_id.as_str(),
                    "app": info.app_name,
                    "key": info.stream_key,
                    "start_time": info.start_time.to_rfc3339(),
                    "video_frames": info.video_frames,
                    "audio_frames": info.audio_frames,
                })
            })
            .collect()
    } else {
        Vec::new()
    };

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
struct HlsPlaylistQuery {
    /// 开始时间偏移（秒，负数表示从现在往前）
    start_time: Option<i64>,
}

async fn hls_playlist(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<HlsPlaylistQuery>,
) -> std::result::Result<Response, StatusCode> {
    use chrono::{Duration, Utc};
    
    // stream_id 格式: "rtmp/live/test123" -> app="live", key="test123"
    let parts: Vec<&str> = stream_id.split('/').collect();
    if parts.len() < 3 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let app_name = parts[1];
    let stream_key = parts[2];

    // 计算开始时间
    let start_time = query.start_time.map(|offset| {
        Utc::now() + Duration::seconds(offset) // offset 为负数表示过去
    });

    let playlist = state.hls_manager
        .get_playlist_with_timeshift(app_name, stream_key, start_time)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let mut resp = Response::new(playlist.into());
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/vnd.apple.mpegurl"),
    );
    Ok(resp)
}

async fn hls_segment(
    State(state): State<AppState>,
    Path((stream_id, segment)): Path<(String, String)>,
) -> std::result::Result<Response, StatusCode> {
    // stream_id 格式: "rtmp/live/test123" -> app="live", key="test123"
    let parts: Vec<&str> = stream_id.split('/').collect();
    if parts.len() < 3 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let app_name = parts[1];
    let stream_key = parts[2];

    // 解析 segment 文件名，提取序号
    // segment_0.ts -> 0
    let sequence = segment
        .trim_end_matches(".ts")
        .trim_start_matches("segment_")
        .parse::<u64>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let ts_data = state.hls_manager
        .get_segment(app_name, stream_key, sequence)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let mut resp: Response = ts_data.into_response();
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("video/mp2t"),
    );
    Ok(resp)
}

async fn http_flv(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> std::result::Result<Response, StatusCode> {
    use axum::body::Body;
    use flux_media_core::playback::flv::{FlvMuxer, FlvTag, FlvTagType};
    
    // 解析 stream_id: "rtmp/live/test123" -> app="live", key="test123"
    let parts: Vec<&str> = stream_id.split('/').collect();
    if parts.len() < 3 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let app_name = parts[1];
    let stream_key = parts[2];

    // 从 StreamManager 订阅流
    let (mut video_rx, mut audio_rx) = state.stream_manager
        .subscribe(app_name, stream_key)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    info!(target: "http_flv", 
        app = app_name, 
        key = stream_key, 
        "HTTP-FLV client connected"
    );

    // 创建 FLV 流
    let stream = async_stream::stream! {
        let mut flv_muxer = FlvMuxer::new();
        
        // 1. 发送 FLV Header
        let header = flv_muxer.generate_header();
        yield Ok::<_, std::io::Error>(header);

        // 2. 循环接收并发送视频/音频数据
        loop {
            tokio::select! {
                Ok(video_packet) = video_rx.recv() => {
                    // 封装视频 Tag
                    let tag = FlvTag {
                        tag_type: FlvTagType::Video,
                        timestamp: video_packet.timestamp,
                        data: video_packet.data,
                    };
                    
                    match flv_muxer.mux_tag(&tag) {
                        Ok(flv_data) => {
                            yield Ok(flv_data);
                        }
                        Err(e) => {
                            error!(target: "http_flv", "Failed to mux video tag: {}", e);
                            break;
                        }
                    }
                }
                Ok(audio_packet) = audio_rx.recv() => {
                    // 封装音频 Tag
                    let tag = FlvTag {
                        tag_type: FlvTagType::Audio,
                        timestamp: audio_packet.timestamp,
                        data: audio_packet.data,
                    };
                    
                    match flv_muxer.mux_tag(&tag) {
                        Ok(flv_data) => {
                            yield Ok(flv_data);
                        }
                        Err(e) => {
                            error!(target: "http_flv", "Failed to mux audio tag: {}", e);
                            break;
                        }
                    }
                }
                else => {
                    // 所有通道都关闭，退出
                    info!(target: "http_flv", "Stream ended");
                    break;
                }
            }
        }
    };

    // 创建响应
    let body = Body::from_stream(stream);
    
    let mut resp = Response::new(body);
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("video/x-flv"),
    );
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
    );
    resp.headers_mut().insert(
        axum::http::header::PRAGMA,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    resp.headers_mut().insert(
        "Access-Control-Allow-Origin",
        axum::http::HeaderValue::from_static("*"),
    );

    Ok(resp)
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

    // 创建媒体处理器
    let media_processor = Arc::new(media_processor::MediaProcessor::new(
        storage.clone(),
        orchestrator.clone(),
    ));

    // 创建流管理器
    let stream_manager = Arc::new(stream_manager::StreamManager::new());

    // 创建时移管理器
    use flux_media_core::timeshift::{TimeShiftCore, TimeShiftConfig};
    let timeshift_config = TimeShiftConfig::default();
    let timeshift = Arc::new(TimeShiftCore::new(
        timeshift_config,
        PathBuf::from("./data/timeshift")
    ));
    
    // 创建 HLS 管理器（集成时移）
    let hls_dir = PathBuf::from("./data/hls");
    let hls_manager = Arc::new(hls_manager::HlsManager::with_timeshift(
        hls_dir,
        Some(timeshift)
    ));

    // 启动 RTMP 服务器
    let rtmp_server = Arc::new(RtmpServer::new(
        args.rtmp_bind.clone(),
        media_processor,
        stream_manager.clone(),
        hls_manager.clone(),
    ));
    
    let state = AppState {
        storage,
        orchestrator,
        streams: Arc::new(RwLock::new(HashMap::new())),
        hls_generators: Arc::new(RwLock::new(HashMap::new())),
        rtmp_server: Some(rtmp_server.clone()),
        hls_manager: hls_manager.clone(),
        stream_manager: stream_manager.clone(),
    };

    let rtmp_task = rtmp_server.clone();
    tokio::spawn(async move {
        if let Err(e) = rtmp_task.start().await {
            tracing::error!(target: "rtmpd", "RTMP server error: {}", e);
        }
    });

    // 启动 HTTP API 服务器
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/rtmp/streams", get(list_streams))
        .route("/api/v1/rtmp/streams/:stream_id/snapshot", get(snapshot))
        .route("/hls/:stream_id/index.m3u8", get(hls_playlist))
        .route("/hls/:stream_id/:segment", get(hls_segment))
        .route("/flv/:stream_id.flv", get(http_flv))
        .with_state(state);

    let addr = args.http_bind;
    tracing::info!(target: "rtmpd", "HTTP API listening on {}", addr);

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
        let stream_id = StreamId::new("rtmp", "live/test123");
        assert_eq!(stream_id.as_str(), "rtmp/live/test123");
        assert_eq!(stream_id.protocol(), Some("rtmp"));
    }
}
