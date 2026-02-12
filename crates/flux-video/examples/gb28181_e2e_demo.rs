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
        sip::{SipServer, SipServerConfig},
    },
    snapshot::KeyframeExtractor,
    storage::StandaloneStorage,
    Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    sip: Arc<SipServer>,
    rtp_receiver: Arc<RtpReceiver>,
    storage: Arc<RwLock<StandaloneStorage>>,
    extractor: Arc<RwLock<KeyframeExtractor>>,
    streams: Arc<RwLock<HashMap<String, Arc<GbStreamProcessor>>>>,
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

    let storage = Arc::new(RwLock::new(StandaloneStorage::new(PathBuf::from("./demo_data/gb28181/storage"))?));
    let extractor = Arc::new(RwLock::new(KeyframeExtractor::new(PathBuf::from("./demo_data/gb28181/keyframes")).with_interval(2)));

    let rtp_receiver = Arc::new(RtpReceiver::new(RtpReceiverConfig {
        bind_addr: "0.0.0.0:9000".to_string(),
        ..Default::default()
    }).await?);

    let sip = Arc::new(SipServer::new(SipServerConfig::default()).await?);

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
        .route(
            "/api/gb28181/devices/:device_id/channels",
            get(list_device_channels),
        )
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
) -> Result<Json<StreamInfo>> {
    let call_id = state
        .sip
        .start_realtime_play(&req.device_id, &req.channel_id, req.rtp_port)
        .await?;

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

    processor.start().await?;

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
) -> Result<Json<ApiResponse>> {
    state.sip.query_catalog(&req.device_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        message: format!("Catalog query sent to device {}", req.device_id),
    }))
}

async fn list_device_channels(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<Vec<ChannelInfo>>> {
    let device = state
        .sip
        .device_manager()
        .get_device(&device_id)
        .await
        .ok_or_else(|| {
            flux_video::VideoError::Other(format!("Device not found: {}", device_id))
        })?;

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
    let streams = state.streams.read().await;
    Json(streams.keys().cloned().collect())
}

async fn snapshot(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Result<Response> {
    let streams = state.streams.read().await;
    let processor = streams
        .get(&stream_id)
        .ok_or_else(|| flux_video::VideoError::StreamNotFound(stream_id.clone()))?;

    let Some(path) = processor.latest_keyframe_path().await else {
        return Err(flux_video::VideoError::Other("No keyframe available".to_string()));
    };

    let data = tokio::fs::read(&path).await
        .map_err(|e| flux_video::VideoError::Other(format!("Failed to read keyframe: {}", e)))?;

    let mut response = Bytes::from(data).into_response();
    response.headers_mut().insert(
        "Content-Type",
        HeaderValue::from_static("application/octet-stream"),
    );

    Ok(response)
}
