use axum::{
    extract::{Path, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use clap::Parser;
use flux_video::{
    gb28181::{
        rtp::{receiver::RtpReceiverConfig, RtpReceiver},
        sip::{SipServer, SipServerConfig},
    },
    Result as VideoResult,
    VideoError,
};
use flux_media_core::{
    snapshot::SnapshotOrchestrator,
    storage::{filesystem::FileSystemStorage, MediaStorage, StorageConfig},
    types::StreamId,
};
use flux_storage::{DiskType, PoolConfig, StorageManager};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

mod telemetry;
use telemetry::TelemetryClient;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(long, default_value = "0.0.0.0:8081")]
    http_bind: String,

    #[arg(long, default_value = "0.0.0.0:5060")]
    sip_bind: String,

    #[arg(long, default_value = "0.0.0.0:9000")]
    rtp_bind: String,

    #[arg(long, default_value = "./data/gb28181/storage")]
    storage_dir: String,

    #[arg(long, default_value = "./data/gb28181/keyframes")]
    keyframe_dir: String,

    #[arg(long, default_value_t = 2)]
    keyframe_interval_secs: u64,

    #[arg(long)]
    telemetry_endpoint: Option<String>,

    #[arg(long, default_value_t = 1000)]
    telemetry_timeout_ms: u64,
}

#[derive(Clone)]
struct AppState {
    sip: Arc<SipServer>,
    rtp_receiver: Arc<RtpReceiver>,
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
    streams: Arc<RwLock<HashMap<String, Arc<GbStreamProcessor>>>>,
    telemetry: TelemetryClient,
}

#[derive(Debug, Deserialize)]
struct InviteRequest {
    device_id: String,
    channel_id: String,
    rtp_port: u16,
}

#[derive(Debug, Deserialize)]
struct GbByeRequest {
    call_id: String,
}

#[derive(Debug, Deserialize)]
struct DeviceRequest {
    device_id: String,
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

struct GbStreamProcessor {
    stream_id: String,
    device_id: String,
    channel_id: String,
    ssrc: u32,
    rtp_port: u16,
    call_id: String,
    rtp_receiver: Arc<RtpReceiver>,
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
    telemetry: TelemetryClient,
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
        storage: Arc<RwLock<FileSystemStorage>>,
        orchestrator: Arc<SnapshotOrchestrator>,
        telemetry: TelemetryClient,
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
            orchestrator,
            telemetry,
            running: Arc::new(RwLock::new(false)),
        }
    }
 async fn start(self: Arc<Self>) -> VideoResult<()> {
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        let mut rtp_rx = self.rtp_receiver.register_stream(self.ssrc).await;

        tracing::info!(
            target: "gb28181d",
            stream_id = %self.stream_id,
            ssrc = self.ssrc,
            rtp_port = self.rtp_port,
            "stream processor started"
        );

        let processor = self.clone();
        tokio::spawn(async move {
            processor.process_loop(&mut rtp_rx).await;
        });

        Ok(())
    }

    async fn process_loop(
        &self,
        rtp_rx: &mut tokio::sync::mpsc::Receiver<flux_video::gb28181::rtp::RtpPacket>,
    ) {
        let mut ps_demuxer = flux_video::gb28181::ps::PsDemuxer::new();
        let mut frame_buffer = Vec::new();

        while *self.running.read().await {
            match tokio::time::timeout(tokio::time::Duration::from_secs(5), rtp_rx.recv()).await {
                Ok(Some(packet)) => {
                    ps_demuxer.input(packet.payload().clone());
                    while let Some(video) = ps_demuxer.pop_video() {
                        frame_buffer.extend_from_slice(&video);
                        if packet.is_marker() && !frame_buffer.is_empty() {
                            if let Err(e) = self.save_frame(&frame_buffer).await {
                                tracing::error!(target: "gb28181d", "save frame failed: {}", e);
                            }
                            if let Err(e) = self.extract_keyframe(&frame_buffer).await {
                                tracing::error!(target: "gb28181d", "extract keyframe failed: {}", e);
                            }
                            frame_buffer.clear();
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    tracing::warn!(target: "gb28181d", stream_id = %self.stream_id, "rtp recv timeout");
                }
            }
        }

        tracing::info!(target: "gb28181d", stream_id = %self.stream_id, "stream processor stopped");
    }

    async fn save_frame(&self, data: &[u8]) -> VideoResult<()> {
        let timestamp = chrono::Utc::now();
        let size = data.len();
        let bytes = Bytes::copy_from_slice(data);
        let stream_id = StreamId::from_string(self.stream_id.clone());
        let mut storage = self.storage.write().await;
        if let Err(e) = storage.put_object(&stream_id, timestamp, bytes).await {
            if self.telemetry.enabled() {
                self.telemetry
                    .post(
                        "storage/write_err",
                        serde_json::json!({
                            "service": "flux-gb28181d",
                            "stream_id": stream_id.as_str(),
                            "error": e.to_string(),
                        }),
                    )
                    .await;
            }

            return Err(VideoError::Other(e.to_string()));
        }

        if self.telemetry.enabled() {
            self.telemetry
                .post_sampled(
                    "storage/write_ok",
                    serde_json::json!({
                        "service": "flux-gb28181d",
                        "stream_id": stream_id.as_str(),
                        "bytes": size,
                    }),
                    200,
                )
                .await;
        }
        Ok(())
    }

    async fn extract_keyframe(&self, data: &[u8]) -> VideoResult<()> {
        let timestamp = chrono::Utc::now();
        let stream_id = StreamId::from_string(self.stream_id.clone());
        self.orchestrator.process_keyframe(&stream_id, data, timestamp).await
            .map_err(|e| VideoError::Other(e.to_string()))?;
        Ok(())
    }

    async fn latest_keyframe_path(&self) -> Option<String> {
        let stream_id = StreamId::from_string(self.stream_id.clone());
        let req = flux_media_core::snapshot::SnapshotRequest {
            stream_id,
            mode: flux_media_core::snapshot::SnapshotMode::Keyframe,
            width: None,
            height: None,
        };
        self.orchestrator.get_snapshot(req).await.ok().map(|_| String::new())
    }
}

fn map_video_error_to_status(err: VideoError) -> StatusCode {
    match err {
        VideoError::StreamNotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let args = Args::parse();

    // 加载配置（用于存储池多池配置）
    let config_loader = flux_config::ConfigLoader::new("./config");

    // 初始化统一存储池（flux-storage）
    let storage_manager = Arc::new(StorageManager::new());
    let pool_configs = match config_loader.load_storage_pools("gb28181") {
        Ok(Some(pools)) => pools,
        Ok(None) => vec![PoolConfig {
            name: "default".to_string(),
            path: PathBuf::from(&args.storage_dir),
            disk_type: DiskType::Unknown,
            priority: 1,
            max_usage_percent: 95.0,
        }],
        Err(e) => {
            tracing::warn!(target: "gb28181d", "Failed to load storage pools config, fallback to CLI storage_dir: {}", e);
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
        tracing::warn!(target: "gb28181d", "StorageManager initialize failed, fallback to local dir: {}", e);
    }
    let selected_root = storage_manager
        .select_pool(0)
        .await
        .unwrap_or_else(|_| PathBuf::from(&args.storage_dir))
        .join("gb28181");

    let telemetry = TelemetryClient::new(args.telemetry_endpoint.clone(), args.telemetry_timeout_ms);
    if telemetry.enabled() {
        telemetry
            .post(
                "storage/service_start",
                serde_json::json!({
                    "service": "flux-gb28181d",
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
    let orchestrator = Arc::new(SnapshotOrchestrator::new(PathBuf::from(&args.keyframe_dir)));

    let rtp_receiver = Arc::new(
        RtpReceiver::new(RtpReceiverConfig {
            bind_addr: args.rtp_bind.clone(),
            ..Default::default()
        })
        .await?,
    );

    let mut sip_cfg = SipServerConfig::default();
    sip_cfg.bind_addr = args.sip_bind.clone();

    let sip = Arc::new(SipServer::new(sip_cfg).await?);

    let sip_task = sip.clone();
    tokio::spawn(async move {
        if let Err(e) = sip_task.start().await {
            tracing::error!(target: "gb28181d", "sip server stopped: {}", e);
        }
    });

    let rtp_task = rtp_receiver.clone();
    tokio::spawn(async move {
        if let Err(e) = rtp_task.start().await {
            tracing::error!(target: "gb28181d", "rtp receiver stopped: {}", e);
        }
    });

    let state = AppState {
        sip,
        rtp_receiver,
        storage,
        orchestrator,
        streams: Arc::new(RwLock::new(HashMap::new())),
        telemetry,
    };

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/v1/gb28181/invite", post(invite))
        .route("/api/v1/gb28181/bye", post(bye))
        .route("/api/v1/gb28181/catalog", post(query_catalog))
        .route("/api/v1/gb28181/device-info", post(query_device_info))
        .route("/api/v1/gb28181/device-status", post(query_device_status))
        .route("/api/v1/gb28181/devices", get(list_devices))
        .route("/api/v1/gb28181/devices/:device_id", get(get_device))
        .route(
            "/api/v1/gb28181/devices/:device_id/channels",
            get(list_device_channels),
        )
        .route(
            "/api/v1/gb28181/streams/:stream_id/snapshot",
            get(snapshot),
        )
        .with_state(state);

    let addr = args.http_bind;
    tracing::info!(target: "gb28181d", "http listening on {}", addr);

    axum::Server::bind(
        &addr
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid http_bind {}: {}", addr, e))?,
    )
    .serve(app.into_make_service())
    .await?;

    Ok(())
}

async fn invite(
    State(state): State<AppState>,
    Json(req): Json<InviteRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let stream_id = format!("gb28181/{}/{}", req.device_id, req.channel_id);

    let call_id = state
        .sip
        .start_realtime_play(&req.device_id, &req.channel_id, req.rtp_port)
        .await
        .map_err(map_video_error_to_status)?;

    let session = state
        .sip
        .session_manager()
        .get_session(&call_id)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let ssrc = session.ssrc.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let processor = Arc::new(GbStreamProcessor::new(
        stream_id.clone(),
        req.device_id,
        req.channel_id,
        ssrc,
        req.rtp_port,
        call_id.clone(),
        state.rtp_receiver.clone(),
        state.storage.clone(),
        state.orchestrator.clone(),
        state.telemetry.clone(),
    ));

    processor
        .clone()
        .start()
        .await
        .map_err(map_video_error_to_status)?;

    let mut streams = state.streams.write().await;
    streams.insert(stream_id.clone(), processor);

    Ok(Json(serde_json::json!({ "call_id": call_id, "stream_id": stream_id, "ssrc": ssrc })))
}

async fn bye(
    State(state): State<AppState>,
    Json(req): Json<GbByeRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    state
        .sip
        .stop_realtime_play(&req.call_id)
        .await
        .map_err(map_video_error_to_status)?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn query_catalog(
    State(state): State<AppState>,
    Json(req): Json<DeviceRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    state
        .sip
        .query_catalog(&req.device_id)
        .await
        .map_err(map_video_error_to_status)?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn query_device_info(
    State(state): State<AppState>,
    Json(req): Json<DeviceRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    state
        .sip
        .query_device_info(&req.device_id)
        .await
        .map_err(map_video_error_to_status)?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn query_device_status(
    State(state): State<AppState>,
    Json(req): Json<DeviceRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    state
        .sip
        .query_device_status(&req.device_id)
        .await
        .map_err(map_video_error_to_status)?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn list_devices(
    State(state): State<AppState>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let devices = state.sip.device_manager().list_devices().await;

    let items: Vec<serde_json::Value> = devices
        .iter()
        .map(|d| {
            serde_json::json!({
                "device_id": d.device_id,
                "name": d.name,
                "ip": d.ip,
                "port": d.port,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "devices": items })))
}

async fn get_device(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let Some(d) = state.sip.device_manager().get_device(&device_id).await else {
        return Err(StatusCode::NOT_FOUND);
    };

    Ok(Json(serde_json::json!({
        "device": {
            "device_id": d.device_id,
            "name": d.name,
            "ip": d.ip,
            "port": d.port,
        }
    })))
}

async fn list_device_channels(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let Some(d) = state.sip.device_manager().get_device(&device_id).await else {
        return Err(StatusCode::NOT_FOUND);
    };

    let channels: Vec<serde_json::Value> = d
        .channels
        .iter()
        .map(|c| {
            serde_json::json!({
                "channel_id": c.channel_id,
                "name": c.name,
                "status": c.status,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "channels": channels })))
}

async fn snapshot(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> std::result::Result<Response, StatusCode> {
    let media_stream_id = StreamId::from_string(stream_id.clone());
    let req = flux_media_core::snapshot::SnapshotRequest {
        stream_id: media_stream_id,
        mode: flux_media_core::snapshot::SnapshotMode::Auto,
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
        HeaderValue::from_static("application/octet-stream"),
    );
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use flux_video::gb28181::sip::{SipRequest, SdpSession};
    use hyper::body::to_bytes;
    use rand::{Rng, SeedableRng};
    use rand::rngs::StdRng;
    use std::net::SocketAddr;
    use tokio::net::UdpSocket;
    use tower::ServiceExt;

    fn create_test_h264_data() -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(&[0, 0, 0, 1]);
        data.push(0x67);
        data.extend_from_slice(&[0x42, 0x00, 0x1f, 0xe9]);

        data.extend_from_slice(&[0, 0, 0, 1]);
        data.push(0x68);
        data.extend_from_slice(&[0xce, 0x3c, 0x80]);

        data.extend_from_slice(&[0, 0, 0, 1]);
        data.push(0x65);
        data.extend_from_slice(&[0x88, 0x84, 0x00, 0x10]);
        data.extend_from_slice(&vec![0xAA; 64]);

        data
    }

    fn build_pes_video_with_pts(payload: &[u8], pts90k: u64) -> Vec<u8> {
        // PES: 00 00 01 E0 + PES_length + flags(3) + optional header + payload
        // 这里实现 PTS-only（5 bytes），更接近真实摄像头。

        let mut pts = [0u8; 5];
        let v = pts90k & 0x1FFF_FFFF;
        pts[0] = 0x21 | (((v >> 29) as u8 & 0x0E) as u8);
        pts[1] = ((v >> 22) & 0xFF) as u8;
        pts[2] = 0x01 | (((v >> 14) as u8) & 0xFE);
        pts[3] = ((v >> 7) & 0xFF) as u8;
        pts[4] = 0x01 | (((v << 1) as u8) & 0xFE);

        let optional_len = 5usize;
        let pes_header_len = 3usize + 1 + optional_len;
        let pes_len = pes_header_len + payload.len();
        let pes_len_u16 = u16::try_from(pes_len).unwrap_or(u16::MAX);

        let mut out = Vec::with_capacity(6 + pes_len);
        out.extend_from_slice(&[0x00, 0x00, 0x01, 0xE0]);
        out.extend_from_slice(&pes_len_u16.to_be_bytes());

        // 0x80: '10' + scrambling/control bits
        // 0x80: PTS_DTS_flags=10
        // 0x05: header_data_length
        out.extend_from_slice(&[0x80, 0x80, optional_len as u8]);
        out.extend_from_slice(&pts);
        out.extend_from_slice(payload);
        out
    }

    fn build_ps_pack_header() -> Vec<u8> {
        // 仅为满足 PsDemuxer::parse_pack_header 的最小长度（14 字节）
        // 00 00 01 BA + 10 bytes (最后一个字节低 3 位作为 stuffing_len，这里设为 0)
        let mut v = vec![0u8; 14];
        v[0] = 0x00;
        v[1] = 0x00;
        v[2] = 0x01;
        v[3] = 0xBA;
        v[13] = 0x00;
        v
    }

    async fn send_ps_over_rtp_fragmented_with_loss_reorder(
        sock: &UdpSocket,
        target: SocketAddr,
        ssrc: u32,
        seq_start: u16,
        timestamp: u32,
        ps: &[u8],
        mtu_payload: usize,
    ) {
        if mtu_payload == 0 {
            return;
        }

        let total_chunks = (ps.len() + mtu_payload - 1) / mtu_payload;
        if total_chunks <= 2 {
            send_ps_over_rtp_fragmented(sock, target, ssrc, seq_start, timestamp, ps, mtu_payload)
                .await;
            return;
        }

        let chunks: Vec<&[u8]> = ps.chunks(mtu_payload).collect();
        let mut seq = seq_start;

        let rtp0 = build_rtp_packet(ssrc, seq, timestamp, false, chunks[0]);
        let _ = sock.send_to(&rtp0, target).await;
        seq = seq.wrapping_add(1);

        let rtp2 = build_rtp_packet(ssrc, seq, timestamp, false, chunks[2]);
        let _ = sock.send_to(&rtp2, target).await;
        seq = seq.wrapping_add(1);

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let rtp1 = build_rtp_packet(ssrc, seq, timestamp, false, chunks[1]);
        let _ = sock.send_to(&rtp1, target).await;
        seq = seq.wrapping_add(1);

        for (i, chunk) in chunks.iter().enumerate().skip(3) {
            if i + 2 == total_chunks {
                continue;
            }
            let is_last = i + 1 == total_chunks;
            let rtp = build_rtp_packet(ssrc, seq, timestamp, is_last, chunk);
            let _ = sock.send_to(&rtp, target).await;
            seq = seq.wrapping_add(1);
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
    }

    fn build_invite_200_ok(invite: &SipRequest) -> String {
        let via = invite.headers.get("Via").cloned().unwrap_or_default();
        let from = invite.headers.get("From").cloned().unwrap_or_default();
        let to = invite.headers.get("To").cloned().unwrap_or_default();
        let call_id = invite.headers.get("Call-ID").cloned().unwrap_or_default();
        let cseq = invite.headers.get("CSeq").cloned().unwrap_or_default();

        format!(
            "SIP/2.0 200 OK\r\n\
Via: {via}\r\n\
From: {from}\r\n\
To: {to}\r\n\
Call-ID: {call_id}\r\n\
CSeq: {cseq}\r\n\
Content-Length: 0\r\n\
\r\n"
        )
    }

    fn build_ps_system_header_min() -> Vec<u8> {
        // 00 00 01 BB + length=0
        vec![0x00, 0x00, 0x01, 0xBB, 0x00, 0x00]
    }

    fn build_ps_program_stream_map_min() -> Vec<u8> {
        // 00 00 01 BC + length=0
        vec![0x00, 0x00, 0x01, 0xBC, 0x00, 0x00]
    }

    fn build_ps_payload_with_video_pes(h264_annexb: &[u8], pts90k: u64) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&build_ps_pack_header());
        v.extend_from_slice(&build_ps_system_header_min());
        v.extend_from_slice(&build_ps_program_stream_map_min());
        v.extend_from_slice(&build_pes_video_with_pts(h264_annexb, pts90k));
        v
    }

    fn build_video_pes_only(h264_annexb: &[u8], pts90k: u64) -> Vec<u8> {
        build_pes_video_with_pts(h264_annexb, pts90k)
    }

    fn build_rtp_packet(ssrc: u32, seq: u16, timestamp: u32, marker: bool, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(12 + payload.len());
        let b0 = 0x80u8;
        let pt = 96u8;
        let b1 = if marker { 0x80 | pt } else { pt };

        v.push(b0);
        v.push(b1);
        v.extend_from_slice(&seq.to_be_bytes());
        v.extend_from_slice(&timestamp.to_be_bytes());
        v.extend_from_slice(&ssrc.to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    async fn send_ps_over_rtp_fragmented(
        sock: &UdpSocket,
        target: SocketAddr,
        ssrc: u32,
        seq_start: u16,
        timestamp: u32,
        ps: &[u8],
        mtu_payload: usize,
    ) {
        if mtu_payload == 0 {
            return;
        }

        let total_chunks = (ps.len() + mtu_payload - 1) / mtu_payload;
        let mut seq = seq_start;

        for (i, chunk) in ps.chunks(mtu_payload).enumerate() {
            let is_last = i + 1 == total_chunks;
            let rtp = build_rtp_packet(ssrc, seq, timestamp, is_last, chunk);
            let _ = sock.send_to(&rtp, target).await;
            seq = seq.wrapping_add(1);
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
    }

    fn build_register(device_id: &str, local_port: u16) -> String {
        let call_id = format!("{}@test", chrono::Utc::now().timestamp());

        format!(
            "REGISTER sip:3402000000 SIP/2.0\r\n\
Via: SIP/2.0/UDP 127.0.0.1:{local_port};branch=z9hG4bK1\r\n\
From: <sip:{device_id}@3402000000>;tag=1\r\n\
To: <sip:{device_id}@3402000000>\r\n\
Call-ID: {call_id}\r\n\
CSeq: 1 REGISTER\r\n\
Expires: 3600\r\n\
Content-Length: 0\r\n\
\r\n",
        )
    }

    #[tokio::test]
    async fn test_e2e_streaming_snapshot() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let storage_dir = temp_dir.path().join("storage");
        let keyframe_dir = temp_dir.path().join("keyframes");

        let storage_config = StorageConfig {
            root_dir: storage_dir,
            retention_days: 7,
            segment_duration_secs: 60,
        };
        let storage = Arc::new(RwLock::new(
            FileSystemStorage::new(storage_config).expect("storage"),
        ));
        let orchestrator = Arc::new(SnapshotOrchestrator::new(keyframe_dir));

        let rtp_receiver = Arc::new(
            RtpReceiver::new(RtpReceiverConfig {
                bind_addr: "127.0.0.1:0".to_string(),
                ..Default::default()
            })
            .await
            .expect("rtp receiver"),
        );
        let rtp_addr = rtp_receiver.local_addr().expect("rtp addr");

        let rtp_task = rtp_receiver.clone();
        tokio::spawn(async move {
            let _ = rtp_task.start().await;
        });

        let mut sip_cfg = SipServerConfig::default();
        sip_cfg.bind_addr = "127.0.0.1:0".to_string();
        let sip = Arc::new(SipServer::new(sip_cfg).await.expect("sip"));
        let sip_addr = sip.local_addr().expect("sip addr");

        let sip_task = sip.clone();
        tokio::spawn(async move {
            let _ = sip_task.start().await;
        });

        let state = AppState {
            sip: sip.clone(),
            rtp_receiver: rtp_receiver.clone(),
            storage,
            orchestrator,
            streams: Arc::new(RwLock::new(HashMap::new())),
        };

        let app = Router::new()
            .route("/api/v1/gb28181/invite", post(invite))
            .route(
                "/api/v1/gb28181/streams/:stream_id/snapshot",
                get(snapshot),
            )
            .with_state(state);

        let device_id = "34020000001320000001";
        let channel_id = "34020000001320000001";

        let dev_sock = UdpSocket::bind("127.0.0.1:0").await.expect("dev sock");
        let dev_local = dev_sock.local_addr().expect("dev local");

        let register = build_register(device_id, dev_local.port());
        dev_sock
            .send_to(register.as_bytes(), sip_addr)
            .await
            .expect("send register");

        let mut buf = vec![0u8; 8192];
        let (n, _from) = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            dev_sock.recv_from(&mut buf),
        )
        .await
        .expect("register resp timeout")
        .expect("register resp");
        let resp_txt = String::from_utf8_lossy(&buf[..n]);
        assert!(resp_txt.starts_with("SIP/2.0 200"));

        let invite_body = serde_json::json!({
            "device_id": device_id,
            "channel_id": channel_id,
            "rtp_port": rtp_addr.port(),
        });
        let req = axum::http::Request::builder()
            .uri("/api/v1/gb28181/invite")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(invite_body.to_string()))
            .expect("req");
        let resp = app.clone().oneshot(req).await.expect("invite resp");
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body()).await.expect("invite body");
        let v: serde_json::Value = serde_json::from_slice(&body).expect("json");
        let call_id = v.get("call_id").and_then(|x| x.as_str()).expect("call_id");
        let stream_id = v.get("stream_id").and_then(|x| x.as_str()).expect("stream_id");
        let ssrc = v.get("ssrc").and_then(|x| x.as_u64()).expect("ssrc") as u32;
        assert!(!call_id.is_empty());

        let (n, _from) = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            dev_sock.recv_from(&mut buf),
        )
        .await
        .expect("invite timeout")
        .expect("invite recv");
        let invite_txt = String::from_utf8_lossy(&buf[..n]);
        let invite_req = SipRequest::from_string(&invite_txt).expect("parse invite");
        assert!(matches!(invite_req.method, flux_video::gb28181::sip::SipMethod::Invite));

        let sdp = invite_req.body.as_deref().expect("sdp");
        let sdp_sess = SdpSession::from_string(sdp).expect("parse sdp");
        assert_eq!(sdp_sess.ssrc, Some(ssrc));

        // 设备侧回复 200 OK，并等待服务器 ACK（更接近真实事务）
        let ok200 = build_invite_200_ok(&invite_req);
        dev_sock
            .send_to(ok200.as_bytes(), sip_addr)
            .await
            .expect("send 200 ok");

        let (n, _from) = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            dev_sock.recv_from(&mut buf),
        )
        .await
        .expect("ack timeout")
        .expect("ack recv");
        let ack_txt = String::from_utf8_lossy(&buf[..n]);
        let ack_req = SipRequest::from_string(&ack_txt).expect("parse ack");
        assert!(matches!(ack_req.method, flux_video::gb28181::sip::SipMethod::Ack));

        let rtp_target: SocketAddr = format!("127.0.0.1:{}", rtp_addr.port())
            .parse()
            .expect("rtp target");

        // 模拟摄像头：多帧 PS 数据，每帧切成多个 RTP 包发送，marker 只在最后一个包。
        let h264 = create_test_h264_data();
        // 连续 PS：仅首帧带 pack/system/psm，后续只发 PES（更接近真实设备）。
        let ps0 = build_ps_payload_with_video_pes(&h264, 90_000);
        let pes1 = build_video_pes_only(&h264, 180_000);
        let pes2 = build_video_pes_only(&h264, 270_000);

        // 关键帧：完整发送（确保至少能产出 snapshot）
        send_ps_over_rtp_fragmented(&dev_sock, rtp_target, ssrc, 1, 100, &ps0, 400).await;
        // 非关键帧：带丢包/乱序模拟
        send_ps_over_rtp_fragmented_with_loss_reorder(&dev_sock, rtp_target, ssrc, 30, 200, &pes1, 200).await;
        // 再发一帧完整 PES
        send_ps_over_rtp_fragmented(&dev_sock, rtp_target, ssrc, 60, 300, &pes2, 250).await;

        let mut ok = false;
        for _ in 0..30 {
            let req = axum::http::Request::builder()
                .uri(format!("/api/v1/gb28181/streams/{}/snapshot", urlencoding::encode(stream_id)))
                .method("GET")
                .body(Body::empty())
                .expect("snapshot req");
            let resp = app.clone().oneshot(req).await.expect("snapshot resp");
            if resp.status() == StatusCode::OK {
                let body = to_bytes(resp.into_body()).await.expect("snapshot body");
                assert!(!body.is_empty());
                ok = true;
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        assert!(ok);
    }

    async fn run_e2e_with_impairments(loss_rate: f64, reorder_rate: f64, seed: u64) -> bool {
        let temp_dir = match tempfile::tempdir() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let storage_dir = temp_dir.path().join("storage");
        let keyframe_dir = temp_dir.path().join("keyframes");

        let storage_config = StorageConfig {
            root_dir: storage_dir,
            retention_days: 7,
            segment_duration_secs: 60,
        };
        let storage = Arc::new(RwLock::new(match FileSystemStorage::new(storage_config) {
            Ok(v) => v,
            Err(_) => return false,
        }));
        let orchestrator = Arc::new(SnapshotOrchestrator::new(keyframe_dir));

        let rtp_receiver = Arc::new(
            match RtpReceiver::new(RtpReceiverConfig {
                bind_addr: "127.0.0.1:0".to_string(),
                ..Default::default()
            })
            .await
            {
                Ok(v) => v,
                Err(_) => return false,
            },
        );
        let rtp_addr = match rtp_receiver.local_addr() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let rtp_task = rtp_receiver.clone();
        tokio::spawn(async move {
            let _ = rtp_task.start().await;
        });

        let mut sip_cfg = SipServerConfig::default();
        sip_cfg.bind_addr = "127.0.0.1:0".to_string();
        let sip = Arc::new(match SipServer::new(sip_cfg).await {
            Ok(v) => v,
            Err(_) => return false,
        });
        let sip_addr = match sip.local_addr() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let sip_task = sip.clone();
        tokio::spawn(async move {
            let _ = sip_task.start().await;
        });

        let state = AppState {
            sip: sip.clone(),
            rtp_receiver: rtp_receiver.clone(),
            storage,
            orchestrator,
            streams: Arc::new(RwLock::new(HashMap::new())),
        };

        let app = Router::new()
            .route("/api/v1/gb28181/invite", post(invite))
            .route(
                "/api/v1/gb28181/streams/:stream_id/snapshot",
                get(snapshot),
            )
            .with_state(state);

        let device_id = format!("3402000000132{:08}", seed % 100_000_000);
        let channel_id = device_id.clone();

        let dev_sock = match UdpSocket::bind("127.0.0.1:0").await {
            Ok(v) => v,
            Err(_) => return false,
        };
        let dev_local = match dev_sock.local_addr() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let register = build_register(&device_id, dev_local.port());
        if dev_sock.send_to(register.as_bytes(), sip_addr).await.is_err() {
            return false;
        }

        let mut buf = vec![0u8; 8192];
        let register_resp = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            dev_sock.recv_from(&mut buf),
        )
        .await;
        let Ok(Ok((n, _))) = register_resp else {
            return false;
        };
        let resp_txt = String::from_utf8_lossy(&buf[..n]);
        if !resp_txt.starts_with("SIP/2.0 200") {
            return false;
        }

        let invite_body = serde_json::json!({
            "device_id": device_id,
            "channel_id": channel_id,
            "rtp_port": rtp_addr.port(),
        });
        let req = axum::http::Request::builder()
            .uri("/api/v1/gb28181/invite")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(invite_body.to_string()))
            .expect("req");
        let resp = match app.clone().oneshot(req).await {
            Ok(v) => v,
            Err(_) => return false,
        };
        if resp.status() != StatusCode::OK {
            return false;
        }
        let body = match to_bytes(resp.into_body()).await {
            Ok(v) => v,
            Err(_) => return false,
        };
        let v: serde_json::Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let stream_id = match v.get("stream_id").and_then(|x| x.as_str()) {
            Some(v) => v.to_string(),
            None => return false,
        };
        let ssrc = match v.get("ssrc").and_then(|x| x.as_u64()) {
            Some(v) => v as u32,
            None => return false,
        };

        let invite_recv = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            dev_sock.recv_from(&mut buf),
        )
        .await;
        let Ok(Ok((n, _))) = invite_recv else {
            return false;
        };
        let invite_txt = String::from_utf8_lossy(&buf[..n]);
        let invite_req = match SipRequest::from_string(&invite_txt) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let ok200 = build_invite_200_ok(&invite_req);
        if dev_sock.send_to(ok200.as_bytes(), sip_addr).await.is_err() {
            return false;
        }

        let ack_recv = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            dev_sock.recv_from(&mut buf),
        )
        .await;
        let Ok(Ok((n, _))) = ack_recv else {
            return false;
        };
        let ack_txt = String::from_utf8_lossy(&buf[..n]);
        let ack_req = match SipRequest::from_string(&ack_txt) {
            Ok(v) => v,
            Err(_) => return false,
        };
        if !matches!(ack_req.method, flux_video::gb28181::sip::SipMethod::Ack) {
            return false;
        }

        let rtp_target: SocketAddr = match format!("127.0.0.1:{}", rtp_addr.port()).parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let h264 = create_test_h264_data();
        let ps0 = build_ps_payload_with_video_pes(&h264, 90_000);
        let pes1 = build_video_pes_only(&h264, 180_000);
        let pes2 = build_video_pes_only(&h264, 270_000);

        let mut rng = StdRng::seed_from_u64(seed);

        let mut ok_sent = false;
        for frame_idx in 0..10u32 {
            let (seq_base, ts) = (1u16.wrapping_add((frame_idx * 30) as u16), 100u32 + frame_idx * 100);
            let payload = if frame_idx == 0 { &ps0 } else if frame_idx % 2 == 0 { &pes2 } else { &pes1 };

            let mtu = if frame_idx == 0 { 400 } else { 220 };
            let chunks: Vec<&[u8]> = payload.chunks(mtu).collect();
            let total = chunks.len();
            let mut packets: Vec<Vec<u8>> = Vec::with_capacity(total);
            for (i, c) in chunks.iter().enumerate() {
                let marker = i + 1 == total;
                let pkt = build_rtp_packet(ssrc, seq_base.wrapping_add(i as u16), ts, marker, c);
                packets.push(pkt);
            }

            let mut send_list: Vec<Vec<u8>> = Vec::new();
            let mut pending: Option<Vec<u8>> = None;
            for pkt in packets {
                if rng.gen::<f64>() < loss_rate {
                    continue;
                }
                if pending.is_none() && rng.gen::<f64>() < reorder_rate {
                    pending = Some(pkt);
                    continue;
                }
                send_list.push(pkt);
                if let Some(p) = pending.take() {
                    send_list.push(p);
                }
            }
            if let Some(p) = pending.take() {
                send_list.push(p);
            }

            for pkt in send_list {
                let _ = dev_sock.send_to(&pkt, rtp_target).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;
            }

            let req = axum::http::Request::builder()
                .uri(format!(
                    "/api/v1/gb28181/streams/{}/snapshot",
                    urlencoding::encode(&stream_id)
                ))
                .method("GET")
                .body(Body::empty())
                .expect("snapshot req");
            let resp = match app.clone().oneshot(req).await {
                Ok(v) => v,
                Err(_) => continue,
            };
            if resp.status() == StatusCode::OK {
                ok_sent = true;
                break;
            }
        }

        ok_sent
    }

    #[tokio::test]
    async fn test_stability_impairment_sweep() {
        let loss_rates = [0.001_f64, 0.005_f64, 0.01_f64, 0.02_f64];
        let reorder_rates = [0.001_f64, 0.005_f64, 0.01_f64, 0.02_f64];

        let mut best = (0.0_f64, 0.0_f64);
        for &loss in &loss_rates {
            for &reorder in &reorder_rates {
                let mut all_ok = true;
                for i in 0..5u64 {
                    let seed = 1000 + (loss.mul_add(100_000.0, 0.0) as u64) * 10 + (reorder.mul_add(100_000.0, 0.0) as u64) + i;
                    if !run_e2e_with_impairments(loss, reorder, seed).await {
                        all_ok = false;
                        break;
                    }
                }
                if all_ok {
                    best = (loss, reorder);
                }
            }
        }

        eprintln!("stability_sweep_best loss_rate={} reorder_rate={}", best.0, best.1);

        // 确保在 2% 以内的组合（子集）稳定通过
        assert!(run_e2e_with_impairments(0.02, 0.02, 4242).await);
        assert!(run_e2e_with_impairments(0.01, 0.02, 4243).await);
        assert!(run_e2e_with_impairments(0.02, 0.01, 4244).await);
    }
}
