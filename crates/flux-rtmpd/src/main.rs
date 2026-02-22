mod auth;
mod hls_manager;
mod http_flv;
mod media_processor;
mod middleware;
mod rtmp_server;
mod rtmp_stream;
mod stream_manager;
mod telemetry;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use auth::login;
use tracing::{error, info};
use clap::Parser;
use flux_media_core::{
    playback::{FlvMuxer, FlvTag, HlsGenerator},
    snapshot::{SnapshotMode, SnapshotOrchestrator, SnapshotRequest},
    storage::{filesystem::FileSystemStorage, StorageConfig},
    types::StreamId,
};
use flux_storage::{DiskType, PoolConfig, StorageManager};
use rtmp_server::RtmpServer;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use telemetry::TelemetryClient;

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
    hls_generators: Arc<RwLock<HashMap<String, Arc<HlsGenerator>>>>,
    rtmp_server: Option<Arc<RtmpServer>>,
    hls_manager: Arc<hls_manager::HlsManager>,
    stream_manager: Arc<stream_manager::StreamManager>,
    unified_stream_manager: Arc<flux_stream::StreamManager>,
    http_flv_server: Arc<http_flv::HttpFlvServer>,
    // 安全组件
    jwt_auth: Arc<flux_middleware::JwtAuth>,
    rbac_manager: Arc<flux_middleware::RbacManager>,
    rate_limiter: Arc<flux_middleware::RateLimiter>,
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

async fn http_flv_route(
    State(state): State<AppState>,
    Path((app_name, stream_key)): Path<(String, String)>,
) -> std::result::Result<Response, StatusCode> {
    // 1. 记录客户端请求到统一流管理器
    use flux_stream::{ClientInfo, ClientType, Protocol};
    
    let client_info = ClientInfo {
        client_id: format!("flv-{}", uuid::Uuid::new_v4()),
        client_type: ClientType::WebBrowser,
        preferred_protocol: Protocol::HttpFlv,
        bandwidth_estimate: None,
        user_agent: None,
    };
    
    let stream_id = flux_media_core::types::StreamId::new(
        "rtmp", 
        &format!("{}/{}", app_name, stream_key)
    );
    
    // 请求输出流（会自动检测是否需要转码）
    if let Err(e) = state.unified_stream_manager
        .request_output(&stream_id, client_info).await 
    {
        tracing::warn!(target: "http_flv", "Failed to request output: {}", e);
    }
    
    // 2. 调用 HttpFlvServer 处理
    let stream_key_clean = stream_key.trim_end_matches(".flv");
    state.http_flv_server.handle_stream(app_name, stream_key_clean.to_string()).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    // 加载配置（用于存储池多池配置）
    let config_loader = flux_config::ConfigLoader::new("./config");

    // 初始化存储
    // 初始化统一存储池（flux-storage）
    let storage_manager = Arc::new(StorageManager::new());
    let pool_configs = match config_loader.load_storage_pools("rtmp") {
        Ok(Some(pools)) => pools,
        Ok(None) => vec![PoolConfig {
            name: "default".to_string(),
            path: PathBuf::from(&args.storage_dir),
            disk_type: DiskType::Unknown,
            priority: 1,
            max_usage_percent: 95.0,
        }],
        Err(e) => {
            tracing::warn!(target: "rtmpd", "Failed to load storage pools config, fallback to CLI storage_dir: {}", e);
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
        tracing::warn!(target: "rtmpd", "StorageManager initialize failed, fallback to local dir: {}", e);
    }

    let selected_root = storage_manager
        .select_pool(0)
        .await
        .unwrap_or_else(|_| PathBuf::from(&args.storage_dir))
        .join("rtmp");

    let telemetry = TelemetryClient::new(args.telemetry_endpoint.clone(), args.telemetry_timeout_ms);
    if telemetry.enabled() {
        telemetry
            .post(
                "storage/service_start",
                serde_json::json!({
                    "service": "flux-rtmpd",
                    "selected_root": selected_root.to_string_lossy().to_string(),
                    "storage_dir": args.storage_dir,
                    "keyframe_dir": args.keyframe_dir,
                }),
            )
            .await;
    }

    let storage_config = StorageConfig {
        root_dir: selected_root,
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
        telemetry.clone(),
    ));

    // 创建 RTMP 流管理器（用于 broadcast channel）
    let stream_manager = Arc::new(stream_manager::StreamManager::new());

    // 创建统一流管理器（协议无关）
    use flux_config::StreamingConfig;
    let streaming_config = StreamingConfig::default();
    let unified_stream_manager = Arc::new(flux_stream::StreamManager::new(streaming_config));

    // 创建时移管理器
    use flux_media_core::timeshift::{TimeShiftCore, TimeShiftConfig};
    let timeshift_config = TimeShiftConfig::default();
    let timeshift = Arc::new(TimeShiftCore::new(
        timeshift_config,
        PathBuf::from("./data/timeshift")
    ));
    
    // 创建 HLS 管理器（集成时移）
    let hls_dir = PathBuf::from("./data/hls");
    let hls_manager = Arc::new(hls_manager::HlsManager::with_storage_manager(
        hls_dir,
        storage_manager,
        Some(timeshift),
        telemetry.clone(),
    ));

    // 创建 HTTP-FLV 服务器
    let http_flv_server = Arc::new(http_flv::HttpFlvServer::new(stream_manager.clone()));

    // 初始化安全组件
    tracing::info!(target: "rtmpd", "Initializing security components");
    
    // JWT 认证（24小时过期）
    let jwt_auth = Arc::new(flux_middleware::JwtAuth::new(
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "flux-iot-secret-change-in-production".to_string()),
        24,
    ));
    
    // RBAC 权限管理
    let rbac_manager = Arc::new(flux_middleware::RbacManager::new());
    
    // 限流器配置
    let rate_limiter = Arc::new(flux_middleware::RateLimiter::new(vec![
        flux_middleware::RateLimitStrategy::by_ip(100, 60),      // 每分钟100个请求/IP
        flux_middleware::RateLimitStrategy::global(10000, 60),   // 全局每分钟10000个请求
        flux_middleware::RateLimitStrategy::by_resource(1000),   // 每个流最多1000个客户端
    ]));

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
        unified_stream_manager: unified_stream_manager.clone(),
        http_flv_server,
        jwt_auth,
        rbac_manager,
        rate_limiter,
    };

    let rtmp_task = rtmp_server.clone();
    tokio::spawn(async move {
        if let Err(e) = rtmp_task.start().await {
            tracing::error!(target: "rtmpd", "RTMP server error: {}", e);
        }
    });

    // 启动 HTTP API 服务器
    tracing::info!(target: "rtmpd", "Setting up HTTP API with security middleware");
    
    // 公开路由（无需认证）
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/login", post(login));  // 登录接口
    
    // 受保护的 API 路由（需要认证和权限）
    let protected_api = Router::new()
        .route("/api/v1/rtmp/streams", get(list_streams))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                middleware::require_permission("streams", "read")
            ))
        .route("/api/v1/rtmp/streams/:stream_id/snapshot", get(snapshot))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                middleware::require_permission("streams", "read")
            ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::jwt_auth_middleware
        ));
    
    // 流媒体路由（限流保护）
    let streaming_routes = Router::new()
        .route("/hls/:stream_id/index.m3u8", get(hls_playlist))
        .route("/hls/:stream_id/:segment", get(hls_segment))
        .route("/flv/:app/:stream.flv", get(http_flv_route))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::rate_limit_middleware
        ));
    
    // 合并所有路由
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_api)
        .merge(streaming_routes)
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
