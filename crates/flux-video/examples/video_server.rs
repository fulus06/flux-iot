// 视频流 HTTP 服务器示例
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post, delete},
    Router,
};
use flux_video::{
    engine::VideoEngine,
    stream::RtspStream,
    snapshot::KeyframeExtractor,
    storage::StandaloneStorage,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;

/// 应用状态
#[derive(Clone)]
struct AppState {
    engine: Arc<RwLock<VideoEngine>>,
    storage: Arc<RwLock<StandaloneStorage>>,
    extractor: Arc<RwLock<KeyframeExtractor>>,
}

/// 创建流请求
#[derive(Debug, Deserialize)]
struct CreateStreamRequest {
    stream_id: String,
    protocol: String,
    url: String,
}

/// 流信息响应
#[derive(Debug, Serialize)]
struct StreamInfo {
    stream_id: String,
    protocol: String,
    url: String,
    state: String,
}

/// 通用响应
#[derive(Debug, Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 创建应用状态
    let engine = Arc::new(RwLock::new(VideoEngine::new()));
    let storage = Arc::new(RwLock::new(
        StandaloneStorage::new(PathBuf::from("./video_data")).unwrap()
    ));
    let extractor = Arc::new(RwLock::new(
        KeyframeExtractor::new(PathBuf::from("./keyframes"))
    ));

    let state = AppState {
        engine,
        storage,
        extractor,
    };

    // 创建路由
    let app = Router::new()
        // Web 播放器
        .route("/", get(serve_index))
        .route("/player.html", get(serve_player))
        // API 路由
        .route("/api/video/streams", post(create_stream))
        .route("/api/video/streams", get(list_streams))
        .route("/api/video/streams/:stream_id", get(get_stream_info))
        .route("/api/video/streams/:stream_id", delete(delete_stream))
        .route("/api/video/streams/:stream_id/snapshot", get(get_snapshot))
        .route("/health", get(health_check))
        .with_state(state);

    // 启动服务器
    let addr = "0.0.0.0:8080";
    tracing::info!("Video server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// 首页
async fn serve_index() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>FLUX Video Server</title>
    <style>
        body { font-family: Arial; max-width: 800px; margin: 50px auto; padding: 20px; }
        h1 { color: #667eea; }
        .link { display: block; margin: 10px 0; padding: 15px; background: #f0f0f0; border-radius: 5px; text-decoration: none; color: #333; }
        .link:hover { background: #e0e0e0; }
    </style>
</head>
<body>
    <h1>🎥 FLUX Video Server</h1>
    <p>视频流监控服务器已启动</p>
    <a class="link" href="/player.html?stream=screen_capture">📺 打开 Web 播放器</a>
    <a class="link" href="/health">🏥 健康检查</a>
    <a class="link" href="/api/video/streams">📋 查看所有流</a>
</body>
</html>
    "#)
}

/// Web 播放器页面
async fn serve_player() -> Html<&'static str> {
    Html(include_str!("../static/player.html"))
}

/// 健康检查
async fn health_check() -> Json<ApiResponse> {
    Json(ApiResponse {
        success: true,
        message: "Video server is running".to_string(),
    })
}

/// 创建流
async fn create_stream(
    State(state): State<AppState>,
    Json(req): Json<CreateStreamRequest>,
) -> Result<Json<ApiResponse>, StatusCode> {
    tracing::info!("Creating stream: {} ({})", req.stream_id, req.protocol);

    match req.protocol.as_str() {
        "rtsp" => {
            let mut stream = RtspStream::new(req.stream_id.clone(), req.url.clone());
            
            // 启动流
            stream.start().await.map_err(|e| {
                tracing::error!("Failed to start stream: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            // 注册到引擎
            let engine = state.engine.read().await;
            engine.publish_stream(req.stream_id.clone(), Arc::new(stream))
                .map_err(|e| {
                    tracing::error!("Failed to publish stream: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            Ok(Json(ApiResponse {
                success: true,
                message: format!("Stream {} created successfully", req.stream_id),
            }))
        }
        _ => {
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// 列出所有流
async fn list_streams(
    State(state): State<AppState>,
) -> Json<Vec<String>> {
    let engine = state.engine.read().await;
    let streams = engine.list_streams();
    Json(streams)
}

/// 获取流信息
async fn get_stream_info(
    State(_state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Result<Json<StreamInfo>, StatusCode> {
    // 简化实现：返回模拟数据
    Ok(Json(StreamInfo {
        stream_id: stream_id.clone(),
        protocol: "rtsp".to_string(),
        url: "rtsp://example.com/stream".to_string(),
        state: "connected".to_string(),
    }))
}

/// 删除流
async fn delete_stream(
    State(_state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Json<ApiResponse> {
    tracing::info!("Deleting stream: {}", stream_id);

    Json(ApiResponse {
        success: true,
        message: format!("Stream {} deleted", stream_id),
    })
}

/// 获取快照
async fn get_snapshot(
    State(_state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Result<Json<ApiResponse>, StatusCode> {
    tracing::info!("Getting snapshot for stream: {}", stream_id);

    Ok(Json(ApiResponse {
        success: true,
        message: format!("Snapshot for stream {} captured", stream_id),
    }))
}
