// GB28181 端到端演示：SIP 注册 + INVITE + RTP/PS 入库 + HTTP 回放/截图

use axum::{
    extract::{Path, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use flux_video::{
    gb28181::{
        rtp::{receiver::RtpReceiverConfig, RtpReceiver},
        sip::{RegisterAuthMode, SipServer, SipServerConfig},
    },
    snapshot::KeyframeExtractor,
    storage::StandaloneStorage,
    Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    sip: Arc<SipServer>,
    rtp_receiver: Arc<RtpReceiver>,
    storage: Arc<RwLock<StandaloneStorage>>,
    extractor: Arc<RwLock<KeyframeExtractor>>,
    streams: Arc<RwLock<HashMap<String, Arc<GbStreamProcessor>>>>,
}

fn load_demo_config(path: &str) -> DemoConfig {
    match fs::read_to_string(path) {
        Ok(s) => match toml::from_str::<DemoConfig>(&s) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!("Failed to parse auth config {}: {}", path, e);
                DemoConfig::default()
            }
        },
        Err(e) => {
            tracing::info!("Auth config {} not found or unreadable, using defaults: {}", path, e);
            DemoConfig::default()
        }
    }
}

fn apply_auth_config_to_sip(auth: &AuthFileConfig, cfg: &mut SipServerConfig) {
    // 映射模式字符串到 RegisterAuthMode
    if let Some(mode_str) = auth.mode.as_deref() {
        let mode = match mode_str.to_ascii_lowercase().as_str() {
            "none" => RegisterAuthMode::None,
            "global" => RegisterAuthMode::Global,
            "per_device" => RegisterAuthMode::PerDevice,
            "global_or_per_device" => RegisterAuthMode::GlobalOrPerDevice,
            other => {
                tracing::warn!("Unknown auth.mode '{}', fallback to None", other);
                RegisterAuthMode::None
            }
        };
        cfg.auth_mode = mode;
    }

    if let Some(pwd) = &auth.global_password {
        cfg.auth_password = Some(pwd.clone());
    }

    if !auth.devices.is_empty() {
        cfg.per_device_passwords = auth.devices.clone();
    }
}

#[derive(Debug, Deserialize, Default)]
struct AuthFileConfig {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    global_password: Option<String>,
    #[serde(default)]
    devices: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Default)]
struct DemoConfig {
    #[serde(default)]
    auth: AuthFileConfig,
}

#[derive(Debug, Deserialize)]
struct InviteRequest {
    device_id: String,
    channel_id: String,
    ssrc: u32,
    rtp_port: u16,
    stream_id: String,
}

#[derive(Debug, Deserialize)]
struct CatalogRequest {
    device_id: String,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct StreamInfo {
    stream_id: String,
    device_id: String,
    channel_id: String,
    ssrc: u32,
    rtp_port: u16,
    call_id: String,
}

#[derive(Debug, Serialize)]
struct ChannelInfo {
    channel_id: String,
    name: String,
    status: String,
    parent_id: String,
    longitude: Option<f64>,
    latitude: Option<f64>,
}

struct GbStreamProcessor {
    stream_id: String,
    device_id: String,
    channel_id: String,
    ssrc: u32,
    rtp_port: u16,
    call_id: String,
    rtp_receiver: Arc<RtpReceiver>,
    storage: Arc<RwLock<StandaloneStorage>>,
    extractor: Arc<RwLock<KeyframeExtractor>>,
    latest_keyframe: Arc<RwLock<Option<String>>>,
    running: Arc<RwLock<bool>>,
}

impl GbStreamProcessor {
    fn new(
        stream_id: String,
        device_id: String,
        channel_id: String,
        ssrc: u32,
        rtp_port: u16,
        call_id: String,
        rtp_receiver: Arc<RtpReceiver>,
        storage: Arc<RwLock<StandaloneStorage>>,
        extractor: Arc<RwLock<KeyframeExtractor>>,
    ) -> Self {
        Self {
            stream_id,
            device_id,
            channel_id,
            ssrc,
            rtp_port,
            call_id,
            rtp_receiver,
            storage,
            extractor,
            latest_keyframe: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    async fn start(self: Arc<Self>) -> Result<()> {
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        let mut rtp_rx = self.rtp_receiver.register_stream(self.ssrc).await;

        tracing::info!(
            "GB28181 stream processor started: {} (SSRC={}, RTP={})",
            self.stream_id,
            self.ssrc,
            self.rtp_port
        );

        let processor = self.clone();
        tokio::spawn(async move {
            processor.process_loop(&mut rtp_rx).await;
        });

        Ok(())
    }

    async fn process_loop(&self, rtp_rx: &mut tokio::sync::mpsc::Receiver<flux_video::gb28181::rtp::RtpPacket>) {
        let mut ps_demuxer = flux_video::gb28181::ps::PsDemuxer::new();
        let mut frame_buffer = Vec::new();

        while *self.running.read().await {
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(5),
                rtp_rx.recv(),
            )
            .await
            {
                Ok(Some(packet)) => {
                    ps_demuxer.input(packet.payload().clone());
                    while let Some(video) = ps_demuxer.pop_video() {
                        frame_buffer.extend_from_slice(&video);
                        if packet.is_marker() && !frame_buffer.is_empty() {
                            if let Err(e) = self.save_frame(&frame_buffer).await {
                                tracing::error!("Save frame failed: {}", e);
                            }
                            if let Err(e) = self.extract_keyframe(&frame_buffer).await {
                                tracing::error!("Extract keyframe failed: {}", e);
                            }
                            frame_buffer.clear();
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    tracing::warn!("RTP receive timeout for stream {}", self.stream_id);
                }
            }
        }

        tracing::info!("GB28181 stream processor stopped: {}", self.stream_id);
    }

    async fn save_frame(&self, data: &[u8]) -> Result<()> {
        let timestamp = chrono::Utc::now();
        let bytes = Bytes::copy_from_slice(data);
        let mut storage = self.storage.write().await;
        storage.put_object(&self.stream_id, timestamp, bytes).await?;
        Ok(())
    }

    async fn extract_keyframe(&self, data: &[u8]) -> Result<()> {
        let timestamp = chrono::Utc::now();
        let mut extractor = self.extractor.write().await;
        if let Some(keyframe) = extractor.process(&self.stream_id, data, timestamp).await? {
            let mut latest = self.latest_keyframe.write().await;
            *latest = Some(keyframe.file_path.clone());
        }
        Ok(())
    }

    async fn latest_keyframe_path(&self) -> Option<String> {
        self.latest_keyframe.read().await.clone()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let demo_cfg = load_demo_config("./demo_data/gb28181/auth.toml");

    let storage = Arc::new(RwLock::new(StandaloneStorage::new(PathBuf::from("./demo_data/gb28181/storage"))?));
    let extractor = Arc::new(RwLock::new(KeyframeExtractor::new(PathBuf::from("./demo_data/gb28181/keyframes")).with_interval(2)));

    let rtp_receiver = Arc::new(RtpReceiver::new(RtpReceiverConfig {
        bind_addr: "0.0.0.0:9000".to_string(),
        ..Default::default()
    }).await?);

    let mut sip_cfg = SipServerConfig::default();
    apply_auth_config_to_sip(&demo_cfg.auth, &mut sip_cfg);
    let sip = Arc::new(SipServer::new(sip_cfg).await?);

    let sip_task = sip.clone();
    tokio::spawn(async move {
        if let Err(e) = sip_task.start().await {
            tracing::error!("SIP server stopped: {}", e);
        }
    });

    let rtp_task = rtp_receiver.clone();
    tokio::spawn(async move {
        if let Err(e) = rtp_task.start().await {
            tracing::error!("RTP receiver stopped: {}", e);
        }
    });

    let state = AppState {
        sip,
        rtp_receiver,
        storage,
        extractor,
        streams: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/gb28181/invite", post(start_invite))
        .route("/api/gb28181/catalog", post(query_catalog))
        .route("/api/gb28181/device-info", post(query_device_info))
        .route("/api/gb28181/device-status", post(query_device_status))
        .route(
            "/api/gb28181/devices/:device_id/channels",
            get(list_device_channels),
        )
        .route("/api/gb28181/devices/:device_id", get(get_device_detail))
        .route("/api/gb28181/streams", get(list_streams))
        .route("/api/gb28181/streams/:stream_id/snapshot", get(snapshot))
        .with_state(state);

    let addr = "0.0.0.0:8081";
    tracing::info!("GB28181 demo HTTP server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await
        .map_err(|e| flux_video::VideoError::Other(format!("Failed to bind HTTP server: {}", e)))?;
    axum::serve(listener, app).await
        .map_err(|e| flux_video::VideoError::Other(format!("HTTP server stopped: {}", e)))?;

    Ok(())
}

async fn health() -> Json<ApiResponse> {
    Json(ApiResponse {
        success: true,
        message: "GB28181 demo server is running".to_string(),
    })
}

async fn start_invite(
    State(state): State<AppState>,
    Json(req): Json<InviteRequest>,
) -> std::result::Result<Json<StreamInfo>, StatusCode> {
    let span = tracing::info_span!(
        "http.gb28181.invite",
        device_id = %req.device_id,
        channel_id = %req.channel_id,
        stream_id = %req.stream_id,
        ssrc = req.ssrc,
        rtp_port = req.rtp_port,
    );
    let _enter = span.enter();

    let call_id = state
        .sip
        .start_realtime_play(&req.device_id, &req.channel_id, req.rtp_port)
        .await
        .map_err(map_video_error_to_status)?;

    let processor = Arc::new(GbStreamProcessor::new(
        req.stream_id.clone(),
        req.device_id.clone(),
        req.channel_id.clone(),
        req.ssrc,
        req.rtp_port,
        call_id.clone(),
        state.rtp_receiver.clone(),
        state.storage.clone(),
        state.extractor.clone(),
    ));

    let processor_to_start = Arc::clone(&processor);

    processor_to_start
        .start()
        .await
        .map_err(map_video_error_to_status)?;

    let mut streams = state.streams.write().await;
    streams.insert(req.stream_id.clone(), processor);

    Ok(Json(StreamInfo {
        stream_id: req.stream_id,
        device_id: req.device_id,
        channel_id: req.channel_id,
        ssrc: req.ssrc,
        rtp_port: req.rtp_port,
        call_id,
    }))
}

async fn query_catalog(
    State(state): State<AppState>,
    Json(req): Json<CatalogRequest>,
) -> std::result::Result<Json<ApiResponse>, StatusCode> {
    let span = tracing::info_span!(
        "http.gb28181.catalog",
        device_id = %req.device_id,
    );
    let _enter = span.enter();

    state
        .sip
        .query_catalog(&req.device_id)
        .await
        .map_err(map_video_error_to_status)?;

    Ok(Json(ApiResponse {
        success: true,
        message: format!("Catalog query sent to device {}", req.device_id),
    }))
}

async fn query_device_info(
    State(state): State<AppState>,
    Json(req): Json<CatalogRequest>,
) -> std::result::Result<Json<ApiResponse>, StatusCode> {
    let span = tracing::info_span!(
        "http.gb28181.device_info",
        device_id = %req.device_id,
    );
    let _enter = span.enter();

    state
        .sip
        .query_device_info(&req.device_id)
        .await
        .map_err(map_video_error_to_status)?;

    Ok(Json(ApiResponse {
        success: true,
        message: format!("DeviceInfo query sent to device {}", req.device_id),
    }))
}

async fn query_device_status(
    State(state): State<AppState>,
    Json(req): Json<CatalogRequest>,
) -> std::result::Result<Json<ApiResponse>, StatusCode> {
    let span = tracing::info_span!(
        "http.gb28181.device_status",
        device_id = %req.device_id,
    );
    let _enter = span.enter();

    state
        .sip
        .query_device_status(&req.device_id)
        .await
        .map_err(map_video_error_to_status)?;

    Ok(Json(ApiResponse {
        success: true,
        message: format!("DeviceStatus query sent to device {}", req.device_id),
    }))
}

#[derive(Debug, Serialize)]
struct DeviceDetail {
    device_id: String,
    name: String,
    ip: String,
    port: u16,
    status: String,
    manufacturer: String,
    model: String,
    firmware: String,
    channel_count: usize,
}

async fn get_device_detail(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> std::result::Result<Json<DeviceDetail>, StatusCode> {
    let span = tracing::info_span!(
        "http.gb28181.device_detail",
        device_id = %device_id,
    );
    let _enter = span.enter();

    let device = state
        .sip
        .device_manager()
        .get_device(&device_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let status = match device.status {
        flux_video::gb28181::sip::DeviceStatus::Online => "Online",
        flux_video::gb28181::sip::DeviceStatus::Offline => "Offline",
        flux_video::gb28181::sip::DeviceStatus::Registering => "Registering",
    };

    Ok(Json(DeviceDetail {
        device_id: device.device_id,
        name: device.name,
        ip: device.ip,
        port: device.port,
        status: status.to_string(),
        manufacturer: device.manufacturer,
        model: device.model,
        firmware: device.firmware,
        channel_count: device.channels.len(),
    }))
}

async fn list_device_channels(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> std::result::Result<Json<Vec<ChannelInfo>>, StatusCode> {
    let span = tracing::info_span!(
        "http.gb28181.device_channels",
        device_id = %device_id,
    );
    let _enter = span.enter();

    let device = state
        .sip
        .device_manager()
        .get_device(&device_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let channels: Vec<ChannelInfo> = device
        .channels
        .iter()
        .map(|ch| ChannelInfo {
            channel_id: ch.channel_id.clone(),
            name: ch.name.clone(),
            status: ch.status.clone(),
            parent_id: ch.parent_id.clone(),
            longitude: ch.longitude,
            latitude: ch.latitude,
        })
        .collect();

    Ok(Json(channels))
}

async fn list_streams(State(state): State<AppState>) -> Json<Vec<String>> {
    let span = tracing::info_span!("http.gb28181.streams.list");
    let _enter = span.enter();

    let streams = state.streams.read().await;
    Json(streams.keys().cloned().collect())
}

async fn snapshot(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> std::result::Result<Response, StatusCode> {
    let span = tracing::info_span!(
        "http.gb28181.snapshot",
        stream_id = %stream_id,
    );
    let _enter = span.enter();

    let streams = state.streams.read().await;
    let processor = streams
        .get(&stream_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let Some(path) = processor.latest_keyframe_path().await else {
        return Err(StatusCode::NOT_FOUND);
    };

    let data = tokio::fs::read(&path)
        .await
        .map_err(|_e| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut response = Bytes::from(data).into_response();
    response.headers_mut().insert(
        "Content-Type",
        HeaderValue::from_static("application/octet-stream"),
    );

    Ok(response)
}

fn map_video_error_to_status(err: flux_video::VideoError) -> StatusCode {
    match err {
        flux_video::VideoError::StreamNotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
