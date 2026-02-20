use crate::{metrics, AppState};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    http::StatusCode,
    response::IntoResponse,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use config::{Config, File, FileFormat};
use chrono::Utc;
use flux_core::entity::events;
use flux_types::message::Message;
use serde::Deserialize;
use serde_json::Value;
use serde::Serialize;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement, TransactionTrait, Value as SeaValue,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock;

#[derive(Deserialize)]
pub struct EventRequest {
    pub topic: String,
    pub payload: Value,
}

#[derive(Deserialize)]
pub struct StorageTelemetryTroubleshootQuery {
    pub topic_prefix: Option<String>,
    pub window_secs: Option<u64>,
    pub top: Option<usize>,
    pub samples: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct StorageTelemetryErrorSample {
    pub timestamp: i64,
    pub topic: String,
    pub service: String,
    pub stream_id: Option<String>,
    pub error: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct StorageTelemetryTroubleshootResponse {
    pub stats: StorageTelemetryStatsResponse,
    pub error_samples: Vec<StorageTelemetryErrorSample>,
}

pub async fn get_storage_telemetry_troubleshoot(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<StorageTelemetryTroubleshootQuery>,
) -> impl IntoResponse {
    let prefix = q.topic_prefix.clone().unwrap_or_else(|| "storage/".to_string());
    let window_secs = q.window_secs.unwrap_or(300);
    let top = q.top.unwrap_or(20).min(200);
    let samples = q.samples.unwrap_or(10).min(200);

    // 1) Recent write_err samples
    let now_ms = Utc::now().timestamp_millis();
    let since_ms = now_ms.saturating_sub(window_secs as i64 * 1000);

    let rows = match events::Entity::find()
        .filter(events::Column::Topic.eq("storage/write_err"))
        .filter(events::Column::Timestamp.gte(since_ms))
        .order_by_desc(events::Column::Timestamp)
        .limit(samples)
        .all(&state.db)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let mut error_samples = Vec::new();
    for r in rows {
        let service = r
            .payload
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let stream_id = r
            .payload
            .get("stream_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let error = r
            .payload
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        error_samples.push(StorageTelemetryErrorSample {
            timestamp: r.timestamp,
            topic: r.topic,
            service,
            stream_id,
            error,
            payload: r.payload,
        });
    }

    // 2) Compute stats inline with top slicing
    let rows_for_stats = match events::Entity::find()
        .filter(events::Column::Topic.like(format!("{}%", prefix)))
        .filter(events::Column::Timestamp.gte(since_ms))
        .filter(events::Column::Timestamp.lte(now_ms))
        .order_by_desc(events::Column::Timestamp)
        .limit(20000)
        .all(&state.db)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let mut counts: std::collections::HashMap<(String, String), u64> =
        std::collections::HashMap::new();
    for r in &rows_for_stats {
        let service = r
            .payload
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let key = (r.topic.clone(), service);
        let entry = counts.entry(key).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    let mut items = counts
        .into_iter()
        .map(|((topic, service), count)| StorageTelemetryStatsItem { topic, service, count })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.count.cmp(&a.count));

    let mut stats_full = StorageTelemetryStatsResponse {
        topic_prefix: prefix.clone(),
        since: Some(since_ms),
        until: Some(now_ms),
        rows_scanned: rows_for_stats.len() as u64,
        items,
    };

    stats_full.items.truncate(top);

    (
        StatusCode::OK,
        Json(StorageTelemetryTroubleshootResponse {
            stats: stats_full,
            error_samples,
        }),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct StorageTelemetryStatsQuery {
    pub topic_prefix: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub max_rows: Option<u64>,
    pub window_secs: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct StatsCacheKey {
    topic_prefix: String,
    max_rows: u64,
    window_secs: u64,
    bucket: i64,
}

struct StatsCacheEntry {
    expires_at_ms: i64,
    resp: StorageTelemetryStatsResponse,
}

fn stats_cache() -> &'static RwLock<HashMap<StatsCacheKey, StatsCacheEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<StatsCacheKey, StatsCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

#[derive(Debug, Serialize, Clone)]
pub struct StorageTelemetryStatsItem {
    pub topic: String,
    pub service: String,
    pub count: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct StorageTelemetryStatsResponse {
    pub topic_prefix: String,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub rows_scanned: u64,
    pub items: Vec<StorageTelemetryStatsItem>,
}

pub async fn get_storage_telemetry_stats(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<StorageTelemetryStatsQuery>,
) -> impl IntoResponse {
    let prefix = q.topic_prefix.unwrap_or_else(|| "storage/".to_string());
    let max_rows = q.max_rows.unwrap_or(2000).min(20000);

    let now_ms = Utc::now().timestamp_millis();
    let (since, until, window_secs) = if let Some(window_secs) = q.window_secs {
        if window_secs == 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "window_secs must be > 0" })),
            )
                .into_response();
        }

        let window_ms = match (window_secs as i64).checked_mul(1000) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "window_secs too large" })),
                )
                    .into_response();
            }
        };

        let since = now_ms.saturating_sub(window_ms);
        (Some(since), Some(now_ms), Some(window_secs))
    } else {
        (q.since, q.until, None)
    };

    // Cache only applies to window-based queries.
    if let Some(window_secs) = window_secs {
        let bucket = (now_ms / 1000) / window_secs as i64;
        let key = StatsCacheKey {
            topic_prefix: prefix.clone(),
            max_rows,
            window_secs,
            bucket,
        };

        let guard = stats_cache().read().await;
        if let Some(hit) = guard.get(&key) {
            if hit.expires_at_ms > now_ms {
                return (StatusCode::OK, Json(hit.resp.clone())).into_response();
            }
        }
    }

    let mut stmt = events::Entity::find()
        .filter(events::Column::Topic.like(format!("{}%", prefix)))
        .order_by_desc(events::Column::Timestamp)
        .limit(max_rows);

    if let Some(since) = since {
        stmt = stmt.filter(events::Column::Timestamp.gte(since));
    }
    if let Some(until) = until {
        stmt = stmt.filter(events::Column::Timestamp.lte(until));
    }

    let rows = match stmt.all(&state.db).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let mut counts: std::collections::HashMap<(String, String), u64> =
        std::collections::HashMap::new();

    for r in &rows {
        let service = r
            .payload
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let key = (r.topic.clone(), service);
        let entry = counts.entry(key).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    let mut items = counts
        .into_iter()
        .map(|((topic, service), count)| StorageTelemetryStatsItem { topic, service, count })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.count.cmp(&a.count));

    let resp = StorageTelemetryStatsResponse {
        topic_prefix: prefix.clone(),
        since,
        until,
        rows_scanned: rows.len() as u64,
        items,
    };

    // Insert cache for window-based queries.
    if let Some(window_secs) = window_secs {
        let bucket = (now_ms / 1000) / window_secs as i64;
        let key = StatsCacheKey {
            topic_prefix: prefix,
            max_rows,
            window_secs,
            bucket,
        };

        let mut guard = stats_cache().write().await;
        guard.insert(
            key,
            StatsCacheEntry {
                expires_at_ms: now_ms.saturating_add(2000),
                resp: resp.clone(),
            },
        );
    }

    (StatusCode::OK, Json(resp)).into_response()
}

pub async fn gb_snapshot(
    State(state): State<Arc<AppState>>,
    Path(stream_id): Path<String>,
) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable().into_response();
    };

    match backend.snapshot(&stream_id).await {
        Ok(Some(bytes)) => {
            let mut resp: Response = bytes.into_response();
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            resp
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "snapshot not found" })),
        )
            .into_response(),
        Err(e) => map_backend_error(e).into_response(),
    }
}

fn gb_backend_unavailable() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": "gb28181 backend is not configured" })),
    )
}

fn map_backend_error(e: anyhow::Error) -> (StatusCode, Json<serde_json::Value>) {
    let msg = e.to_string();
    let status = if msg.contains("404") || msg.to_ascii_lowercase().contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(serde_json::json!({ "error": msg })))
}

#[cfg(test)]
mod gateway_e2e_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use hyper::body::to_bytes;
    use tokio::net::UdpSocket;
    use flux_video::gb28181::sip::{SipServer, SipServerConfig};
    use flux_video::gb28181::rtp::receiver::RtpReceiverConfig;
    use flux_video::gb28181::rtp::RtpReceiver;
    use flux_video::snapshot::KeyframeExtractor;
    use flux_video::storage::StandaloneStorage;
    use tokio::sync::RwLock;
    use tokio::sync::oneshot;
    use tower::ServiceExt;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn test_gateway_e2e_snapshot_via_remote_gb28181d() {
        #[derive(Clone)]
        struct GbState {
            sip: Arc<SipServer>,
            rtp: Arc<RtpReceiver>,
            storage: Arc<RwLock<StandaloneStorage>>, 
            extractor: Arc<RwLock<KeyframeExtractor>>,
            streams: Arc<RwLock<std::collections::HashMap<String, Arc<GbProc>>>>,
        }

        struct GbProc {
            stream_id: String,
            ssrc: u32,
            rtp: Arc<RtpReceiver>,
            storage: Arc<RwLock<StandaloneStorage>>,
            extractor: Arc<RwLock<KeyframeExtractor>>,
            latest: Arc<RwLock<Option<String>>>,
            running: Arc<RwLock<bool>>,
        }

        impl GbProc {
            async fn start(self: Arc<Self>) {
                {
                    let mut running = self.running.write().await;
                    *running = true;
                }
                
                let mut rx = self.rtp.register_stream(self.ssrc).await;
                let proc2 = self.clone();
                tokio::spawn(async move {
                    let mut demux = flux_video::gb28181::ps::PsDemuxer::new();
                    let mut frame_buffer = Vec::new();
                    
                    while *proc2.running.read().await {
                        let timeout_result = tokio::time::timeout(
                            tokio::time::Duration::from_secs(10),
                            rx.recv()
                        ).await;
                        
                        match timeout_result {
                            Ok(Some(packet)) => {
                                demux.input(packet.payload().clone());
                                while let Some(video) = demux.pop_video() {
                                    frame_buffer.extend_from_slice(&video);
                                    if packet.is_marker() && !frame_buffer.is_empty() {
                                        let ts = chrono::Utc::now();
                                        let bytes = axum::body::Bytes::copy_from_slice(&frame_buffer);
                                        {
                                            let mut st = proc2.storage.write().await;
                                            let _ = st.put_object(&proc2.stream_id, ts, bytes).await;
                                        }
                                        {
                                            let mut ex = proc2.extractor.write().await;
                                            if let Ok(Some(kf)) = ex.process(&proc2.stream_id, &frame_buffer, ts).await {
                                                let path = kf.file_path.clone();
                                                let mut latest = proc2.latest.write().await;
                                                *latest = Some(kf.file_path);
                                                eprintln!("Keyframe extracted: {}", path);
                                            }
                                        }
                                        frame_buffer.clear();
                                    }
                                }
                            }
                            Ok(None) => break,
                            Err(_) => {
                                eprintln!("RTP recv timeout for stream {}", proc2.stream_id);
                            }
                        }
                    }
                    eprintln!("Stream processor stopped for {}", proc2.stream_id);
                });
            }

            async fn latest_path(&self) -> Option<String> {
                self.latest.read().await.clone()
            }
        }

        async fn gb_invite(
            State(state): State<GbState>,
            Json(req): Json<super::GbInviteRequest>,
        ) -> impl IntoResponse {
            let stream_id = format!("gb28181/{}/{}", req.device_id, req.channel_id);
            let call_id = match state
                .sip
                .start_realtime_play(&req.device_id, &req.channel_id, req.rtp_port)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                }
            };

            let session = match state.sip.session_manager().get_session(&call_id).await {
                Some(v) => v,
                None => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": "session not found"})),
                    )
                }
            };

            let ssrc = match session.ssrc {
                Some(v) => v,
                None => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": "ssrc not found"})),
                    )
                }
            };

            let proc = Arc::new(GbProc {
                stream_id: stream_id.clone(),
                ssrc,
                rtp: state.rtp.clone(),
                storage: state.storage.clone(),
                extractor: state.extractor.clone(),
                latest: Arc::new(RwLock::new(None)),
                running: Arc::new(RwLock::new(false)),
            });
            
            {
                let mut map = state.streams.write().await;
                map.insert(stream_id.clone(), proc.clone());
            }
            
            proc.start().await;

            (
                StatusCode::OK,
                Json(serde_json::json!({"call_id": call_id, "stream_id": stream_id, "ssrc": ssrc})),
            )
        }

        async fn gb_snapshot(
            State(state): State<GbState>,
            Path(stream_id): Path<String>,
        ) -> impl IntoResponse {
            let map = state.streams.read().await;
            let Some(proc) = map.get(&stream_id) else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let Some(path) = proc.latest_path().await else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let data = match tokio::fs::read(&path).await {
                Ok(v) => v,
                Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            };
            let mut resp: Response = axum::body::Bytes::from(data).into_response();
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            resp
        }

        let temp_dir = tempfile::tempdir().expect("tempdir");
        let storage_dir = temp_dir.path().join("storage");
        let keyframe_dir = temp_dir.path().join("keyframes");

        let storage = Arc::new(RwLock::new(
            StandaloneStorage::new(storage_dir).expect("storage"),
        ));
        let extractor = Arc::new(RwLock::new(
            KeyframeExtractor::new(keyframe_dir).with_interval(0),
        ));

        let rtp = Arc::new(
            RtpReceiver::new(RtpReceiverConfig {
                bind_addr: "127.0.0.1:0".to_string(),
                ..Default::default()
            })
            .await
            .expect("rtp"),
        );
        let rtp_addr = rtp.local_addr().expect("rtp addr");
        let rtp_task = rtp.clone();
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

        let gb_state = GbState {
            sip,
            rtp,
            storage,
            extractor,
            streams: Arc::new(RwLock::new(std::collections::HashMap::new())),
        };

        let gb_app = Router::new()
            .route("/api/v1/gb28181/invite", post(gb_invite))
            .route(
                "/api/v1/gb28181/streams/:stream_id/snapshot",
                get(gb_snapshot),
            )
            .with_state(gb_state);

        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let gb_addr = std_listener.local_addr().expect("addr");
        std_listener.set_nonblocking(true).expect("nb");
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::Server::from_tcp(std_listener)
                .expect("from_tcp")
                .serve(gb_app.into_make_service())
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                });
            let _ = server.await;
        });
        let base_url = format!("http://{}", gb_addr);

        let gateway_state = super::tests::create_test_state_with_remote_backend(base_url).await;
        let gateway = crate::api::create_router(gateway_state);

        let dev_sock = UdpSocket::bind("127.0.0.1:0").await.expect("dev sock");
        let dev_local = dev_sock.local_addr().expect("dev local");
        let device_id = "34020000001320000001";
        let reg = format!(
            "REGISTER sip:3402000000 SIP/2.0\r\n\
Via: SIP/2.0/UDP 127.0.0.1:{};branch=z9hG4bK1\r\n\
From: <sip:{}@3402000000>;tag=1\r\n\
To: <sip:{}@3402000000>\r\n\
Call-ID: 1@test\r\n\
CSeq: 1 REGISTER\r\n\
Expires: 3600\r\n\
Content-Length: 0\r\n\
\r\n",
            dev_local.port(),
            device_id,
            device_id,
        );
        dev_sock.send_to(reg.as_bytes(), sip_addr).await.expect("reg");
        let mut buf = vec![0u8; 8192];
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), dev_sock.recv_from(&mut buf))
            .await
            .expect("reg resp");

        let invite_body = serde_json::json!({
            "device_id": device_id,
            "channel_id": device_id,
            "rtp_port": rtp_addr.port(),
        });
        let req = Request::builder()
            .uri("/api/v1/gb28181/invite")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(invite_body.to_string()))
            .expect("req");
        let resp = gateway.clone().oneshot(req).await.expect("invite resp");
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body()).await.expect("body");
        let v: serde_json::Value = serde_json::from_slice(&body).expect("json");
        let _call_id = v.get("call_id").and_then(|x| x.as_str()).expect("call_id");
        let stream_id = format!("gb28181/{}/{}", device_id, device_id);

        let (n, from) = tokio::time::timeout(tokio::time::Duration::from_secs(2), dev_sock.recv_from(&mut buf))
            .await
            .expect("invite")
            .expect("invite recv");
        let invite_txt = String::from_utf8_lossy(&buf[..n]);
        let invite_req = flux_video::gb28181::sip::SipRequest::from_string(&invite_txt).expect("invite parse");
        
        let sdp = invite_req.body.as_deref().expect("sdp");
        let sdp_sess = flux_video::gb28181::sip::SdpSession::from_string(sdp).expect("parse sdp");
        let ssrc = sdp_sess.ssrc.expect("ssrc");
        let ok200 = format!(
            "SIP/2.0 200 OK\r\n\
Via: {}\r\n\
From: {}\r\n\
To: {}\r\n\
Call-ID: {}\r\n\
CSeq: {}\r\n\
Content-Length: 0\r\n\
\r\n",
            invite_req.headers.get("Via").cloned().unwrap_or_default(),
            invite_req.headers.get("From").cloned().unwrap_or_default(),
            invite_req.headers.get("To").cloned().unwrap_or_default(),
            invite_req.headers.get("Call-ID").cloned().unwrap_or_default(),
            invite_req.headers.get("CSeq").cloned().unwrap_or_default(),
        );
        dev_sock.send_to(ok200.as_bytes(), sip_addr).await.expect("200ok");
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), dev_sock.recv_from(&mut buf))
            .await
            .expect("ack");

        let h264 = {
            let mut data = Vec::new();
            data.extend_from_slice(&[0, 0, 0, 1, 0x67, 0x42, 0x00, 0x1f, 0xe9]);
            data.extend_from_slice(&[0, 0, 0, 1, 0x68, 0xce, 0x3c, 0x80]);
            data.extend_from_slice(&[0, 0, 0, 1, 0x65, 0x88, 0x84, 0x00, 0x10]);
            data.extend_from_slice(&vec![0xAA; 64]);
            data
        };
        let pes = {
            let mut out = Vec::new();
            let pes_len = 3usize + h264.len();
            let pes_len_u16 = u16::try_from(pes_len).unwrap_or(u16::MAX);
            out.extend_from_slice(&[0x00, 0x00, 0x01, 0xE0]);
            out.extend_from_slice(&pes_len_u16.to_be_bytes());
            out.extend_from_slice(&[0x80, 0x00, 0x00]);
            out.extend_from_slice(&h264);
            out
        };
        let mut rtp_pkt = Vec::new();
        rtp_pkt.extend_from_slice(&[0x80, 0xE0]);
        rtp_pkt.extend_from_slice(&1u16.to_be_bytes());
        rtp_pkt.extend_from_slice(&100u32.to_be_bytes());
        rtp_pkt.extend_from_slice(&ssrc.to_be_bytes());
        rtp_pkt.extend_from_slice(&pes);

        let rtp_target: SocketAddr = format!("127.0.0.1:{}", rtp_addr.port()).parse().expect("rtp");
        
        eprintln!("Sending RTP packets to {} with ssrc={}", rtp_target, ssrc);
        // 发送多帧以确保至少有一帧能成功触发 keyframe 提取
        for i in 0..10 {
            let mut pkt = Vec::new();
            pkt.extend_from_slice(&[0x80, 0xE0]);
            pkt.extend_from_slice(&((i + 1) as u16).to_be_bytes());
            pkt.extend_from_slice(&((100 + i * 100) as u32).to_be_bytes());
            pkt.extend_from_slice(&ssrc.to_be_bytes());
            pkt.extend_from_slice(&pes);
            dev_sock.send_to(&pkt, rtp_target).await.expect("rtp send");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        eprintln!("Waiting for keyframe extraction...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let mut ok = false;
        for attempt in 0..60 {
            let req = Request::builder()
                .uri(format!(
                    "/api/v1/gb28181/streams/{}/snapshot",
                    stream_id.replace("/", "%2F")
                ))
                .method("GET")
                .body(Body::empty())
                .expect("snap req");
            let resp = gateway.clone().oneshot(req).await.expect("snap resp");
            eprintln!("attempt {} status={}", attempt, resp.status());
            if resp.status() == StatusCode::OK {
                let body = to_bytes(resp.into_body()).await.expect("snap body");
                eprintln!("snapshot body len={}", body.len());
                assert!(!body.is_empty());
                ok = true;
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        let _ = shutdown_tx.send(());
        let _ = from;
        assert!(ok, "gateway E2E: snapshot never returned 200 OK");
    }
}

pub async fn get_app_config_audit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_admin_auth(&headers) {
        return e;
    }

    let Some(db) = state.config_db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "config_db is not configured; run with --config-source sqlite|postgres"
            })),
        );
    };

    if let Err(e) = ensure_app_config_audit_table(db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let backend = db.get_database_backend();
    let stmt = Statement::from_string(
        backend,
        "SELECT id, prev_updated_at, new_updated_at, prev_hash, new_hash, user_agent, forwarded_for, created_at \
         FROM app_config_audit ORDER BY created_at DESC LIMIT 50"
            .to_string(),
    );

    let rows = match db.query_all(stmt).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let mut items: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = match row.try_get("", "id") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let prev_updated_at: Option<i64> = match row.try_get("", "prev_updated_at") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let new_updated_at: i64 = match row.try_get("", "new_updated_at") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let prev_hash: Option<String> = match row.try_get("", "prev_hash") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let new_hash: String = match row.try_get("", "new_hash") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let user_agent: Option<String> = match row.try_get("", "user_agent") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let forwarded_for: Option<String> = match row.try_get("", "forwarded_for") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let created_at: i64 = match row.try_get("", "created_at") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };

        items.push(serde_json::json!({
            "id": id,
            "prev_updated_at": prev_updated_at,
            "new_updated_at": new_updated_at,
            "prev_hash": prev_hash,
            "new_hash": new_hash,
            "user_agent": user_agent,
            "forwarded_for": forwarded_for,
            "created_at": created_at,
        }));
    }

    (StatusCode::OK, Json(serde_json::json!({ "items": items })))
}

fn check_reload_policy(old_cfg: &flux_server::AppConfig, new_cfg: &flux_server::AppConfig) -> Option<Vec<&'static str>> {
    let mut blocked: Vec<&'static str> = Vec::new();

    if old_cfg.server.host != new_cfg.server.host {
        blocked.push("server.host");
    }
    if old_cfg.server.port != new_cfg.server.port {
        blocked.push("server.port");
    }
    if old_cfg.database.url != new_cfg.database.url {
        blocked.push("database.url");
    }
    if old_cfg.plugins.directory != new_cfg.plugins.directory {
        blocked.push("plugins.directory");
    }

    // GB28181 SIP 绑定/标识类配置涉及 socket 绑定和协议身份，不允许热更新。
    if old_cfg.gb28181.enabled != new_cfg.gb28181.enabled {
        blocked.push("gb28181.enabled");
    }
    if old_cfg.gb28181.backend != new_cfg.gb28181.backend {
        blocked.push("gb28181.backend");
    }
    if old_cfg.gb28181.remote.base_url != new_cfg.gb28181.remote.base_url {
        blocked.push("gb28181.remote.base_url");
    }
    if old_cfg.gb28181.sip.bind_addr != new_cfg.gb28181.sip.bind_addr {
        blocked.push("gb28181.sip.bind_addr");
    }
    if old_cfg.gb28181.sip.sip_domain != new_cfg.gb28181.sip.sip_domain {
        blocked.push("gb28181.sip.sip_domain");
    }
    if old_cfg.gb28181.sip.sip_id != new_cfg.gb28181.sip.sip_id {
        blocked.push("gb28181.sip.sip_id");
    }
    if old_cfg.gb28181.sip.device_expires != new_cfg.gb28181.sip.device_expires {
        blocked.push("gb28181.sip.device_expires");
    }
    if old_cfg.gb28181.sip.session_timeout != new_cfg.gb28181.sip.session_timeout {
        blocked.push("gb28181.sip.session_timeout");
    }

    if blocked.is_empty() {
        None
    } else {
        Some(blocked)
    }
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub script: String,
}

#[derive(Deserialize)]
pub struct UpdateAppConfigRequest {
    pub content: String,
}

#[derive(Deserialize)]
pub struct GbInviteRequest {
    pub device_id: String,
    pub channel_id: String,
    pub rtp_port: u16,
}

#[derive(Deserialize)]
pub struct GbByeRequest {
    pub call_id: String,
}

#[derive(Deserialize)]
pub struct GbDeviceRequest {
    pub device_id: String,
}

pub async fn gb_invite(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbInviteRequest>,
) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable();
    };

    match backend
        .invite(&req.device_id, &req.channel_id, req.rtp_port)
        .await
    {
        Ok(call_id) => (StatusCode::OK, Json(serde_json::json!({ "call_id": call_id }))),
        Err(e) => map_backend_error(e),
    }
}

pub async fn gb_bye(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbByeRequest>,
) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable();
    };

    match backend.bye(&req.call_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => map_backend_error(e),
    }
}

pub async fn gb_query_catalog(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbDeviceRequest>,
) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable();
    };

    match backend.query_catalog(&req.device_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => map_backend_error(e),
    }
}

pub async fn gb_query_device_info(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbDeviceRequest>,
) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable();
    };

    match backend.query_device_info(&req.device_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => map_backend_error(e),
    }
}

pub async fn gb_query_device_status(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GbDeviceRequest>,
) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable();
    };

    match backend.query_device_status(&req.device_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => map_backend_error(e),
    }
}

pub async fn gb_list_devices(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable();
    };

    match backend.list_devices().await {
        Ok(devices) => (StatusCode::OK, Json(serde_json::json!({ "devices": devices }))),
        Err(e) => map_backend_error(e),
    }
}

pub async fn gb_get_device(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable();
    };

    match backend.get_device(&device_id).await {
        Ok(Some(device)) => (StatusCode::OK, Json(serde_json::json!({ "device": device }))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "device not found" })),
        ),
        Err(e) => map_backend_error(e),
    }
}

pub async fn gb_list_device_channels(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let Some(backend) = state.gb28181_backend.as_ref() else {
        return gb_backend_unavailable();
    };

    match backend.list_device_channels(&device_id).await {
        Ok(Some(channels)) => (StatusCode::OK, Json(serde_json::json!({ "channels": channels }))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "device not found" })),
        ),
        Err(e) => map_backend_error(e),
    }
}

fn require_admin_auth(headers: &HeaderMap) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let token = match std::env::var("FLUX_ADMIN_TOKEN") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "FLUX_ADMIN_TOKEN is not configured"
                })),
            ))
        }
    };

    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let expected = format!("Bearer {}", token);
    if auth != expected {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized" })),
        ));
    }

    Ok(())
}

async fn ensure_app_config_audit_table(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    let backend = db.get_database_backend();

    let sql = match backend {
        DbBackend::Sqlite => {
            "CREATE TABLE IF NOT EXISTS app_config_audit (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                prev_updated_at INTEGER,\
                new_updated_at INTEGER NOT NULL,\
                prev_hash TEXT,\
                new_hash TEXT NOT NULL,\
                user_agent TEXT,\
                forwarded_for TEXT,\
                created_at INTEGER NOT NULL\
            )"
        }
        DbBackend::Postgres => {
            "CREATE TABLE IF NOT EXISTS app_config_audit (\
                id BIGSERIAL PRIMARY KEY,\
                prev_updated_at BIGINT,\
                new_updated_at BIGINT NOT NULL,\
                prev_hash TEXT,\
                new_hash TEXT NOT NULL,\
                user_agent TEXT,\
                forwarded_for TEXT,\
                created_at BIGINT NOT NULL\
            )"
        }
        DbBackend::MySql => {
            "CREATE TABLE IF NOT EXISTS app_config_audit (\
                id BIGINT AUTO_INCREMENT PRIMARY KEY,\
                prev_updated_at BIGINT,\
                new_updated_at BIGINT NOT NULL,\
                prev_hash TEXT,\
                new_hash TEXT NOT NULL,\
                user_agent TEXT,\
                forwarded_for TEXT,\
                created_at BIGINT NOT NULL\
            )"
        }
    };

    db.execute(Statement::from_string(backend, sql.to_string()))
        .await?;
    Ok(())
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

async fn ensure_app_config_table(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    let backend = db.get_database_backend();

    let sql = match backend {
        DbBackend::Sqlite => {
            "CREATE TABLE IF NOT EXISTS app_config (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                content TEXT NOT NULL,\
                updated_at INTEGER NOT NULL\
            )"
        }
        DbBackend::Postgres => {
            "CREATE TABLE IF NOT EXISTS app_config (\
                id BIGSERIAL PRIMARY KEY,\
                content TEXT NOT NULL,\
                updated_at BIGINT NOT NULL\
            )"
        }
        DbBackend::MySql => {
            "CREATE TABLE IF NOT EXISTS app_config (\
                id BIGINT AUTO_INCREMENT PRIMARY KEY,\
                content TEXT NOT NULL,\
                updated_at BIGINT NOT NULL\
            )"
        }
    };

    db.execute(Statement::from_string(backend, sql.to_string()))
        .await?;
    Ok(())
}

pub async fn get_app_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_admin_auth(&headers) {
        return e;
    }

    let Some(db) = state.config_db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "config_db is not configured; run with --config-source sqlite|postgres"
            })),
        );
    };

    if let Err(e) = ensure_app_config_table(db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let backend = db.get_database_backend();
    let stmt = Statement::from_string(
        backend,
        "SELECT content, updated_at FROM app_config ORDER BY updated_at DESC LIMIT 1".to_string(),
    );

    let row_opt = match db.query_one(stmt).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let Some(row) = row_opt else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "no app_config found" })),
        );
    };

    let content: String = match row.try_get("", "content") {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };
    let updated_at: i64 = match row.try_get("", "updated_at") {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({ "content": content, "updated_at": updated_at })),
    )
}

pub async fn update_app_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<UpdateAppConfigRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_admin_auth(&headers) {
        return e;
    }

    let Some(db) = state.config_db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "config_db is not configured; run with --config-source sqlite|postgres"
            })),
        );
    };

    if let Err(e) = ensure_app_config_table(db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    // 先校验 TOML 语法 & 能反序列化为 AppConfig
    let settings = match Config::builder()
        .add_source(File::from_str(&req.content, FileFormat::Toml))
        .build()
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let new_cfg: flux_server::AppConfig = match settings.try_deserialize() {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let old_cfg = state.config.borrow().clone();
    if let Some(blocked) = check_reload_policy(&old_cfg, &new_cfg) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "config contains changes that require restart",
                "blocked": blocked,
            })),
        );
    }

    if let Err(e) = ensure_app_config_audit_table(db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let backend = db.get_database_backend();
    let now = chrono::Utc::now().timestamp_millis();

    let prev_stmt = Statement::from_string(
        backend,
        "SELECT content, updated_at FROM app_config ORDER BY updated_at DESC LIMIT 1".to_string(),
    );

    let prev_row_opt = match db.query_one(prev_stmt).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let (prev_updated_at, prev_hash) = if let Some(row) = prev_row_opt {
        let prev_content: String = match row.try_get("", "content") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };
        let prev_updated_at: i64 = match row.try_get("", "updated_at") {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        };

        (Some(prev_updated_at), Some(sha256_hex(&prev_content)))
    } else {
        (None, None)
    };

    let new_hash = sha256_hex(&req.content);
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let forwarded_for = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let txn = match db.begin().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    let insert_config_sql = match backend {
        DbBackend::Postgres => "INSERT INTO app_config (content, updated_at) VALUES ($1, $2)",
        DbBackend::Sqlite | DbBackend::MySql => {
            "INSERT INTO app_config (content, updated_at) VALUES (?, ?)"
        }
    };
    let stmt = Statement::from_sql_and_values(
        backend,
        insert_config_sql,
        vec![
            SeaValue::String(Some(Box::new(req.content))),
            SeaValue::BigInt(Some(now)),
        ],
    );

    if let Err(e) = txn.execute(stmt).await {
        let _ = txn.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let insert_audit_sql = match backend {
        DbBackend::Postgres => {
            "INSERT INTO app_config_audit (prev_updated_at, new_updated_at, prev_hash, new_hash, user_agent, forwarded_for, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)"
        }
        DbBackend::Sqlite | DbBackend::MySql => {
            "INSERT INTO app_config_audit (prev_updated_at, new_updated_at, prev_hash, new_hash, user_agent, forwarded_for, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        }
    };

    let stmt = Statement::from_sql_and_values(
        backend,
        insert_audit_sql,
        vec![
            SeaValue::BigInt(prev_updated_at),
            SeaValue::BigInt(Some(now)),
            SeaValue::String(prev_hash.map(Box::new)),
            SeaValue::String(Some(Box::new(new_hash.clone()))),
            SeaValue::String(user_agent.map(Box::new)),
            SeaValue::String(forwarded_for.map(Box::new)),
            SeaValue::BigInt(Some(now)),
        ],
    );

    if let Err(e) = txn.execute(stmt).await {
        let _ = txn.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    if let Err(e) = txn.commit().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "updated", "updated_at": now, "hash": new_hash })),
    )
}

// Handler for POST /api/v1/event
async fn accept_event(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EventRequest>,
) -> impl IntoResponse {
    // 记录 HTTP 请求
    metrics::record_http_request();
    let start = std::time::Instant::now();

    let msg = Message::new(req.topic, req.payload);
    let msg_id = msg.id.to_string();

    // Publish to Event Bus
    if let Err(e) = state.event_bus.publish(msg) {
        tracing::warn!(
            "Event published but no subscribers: {} (Error: {})",
            msg_id,
            e
        );
    } else {
        tracing::debug!("Event published: {}", msg_id);
        // 记录事件发布成功
        metrics::record_event_published();
    }

    // 记录请求时长
    let duration = start.elapsed().as_secs_f64();
    metrics::record_http_duration(duration);

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "id": msg_id })),
    )
}

pub async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    metrics::record_http_request();
    let start = std::time::Instant::now();

    tracing::info!("Creating rule: {}", req.name);

    // 1. Compile & Validate (and Cache in ScriptEngine)
    if let Err(e) = state.script_engine.compile_script(&req.name, &req.script) {
        tracing::error!("Failed to compile rule {}: {}", req.name, e);
        metrics::record_http_duration(start.elapsed().as_secs_f64());
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Script compilation failed: {}", e) })),
        );
    }

    // 2. Persist to DB
    use flux_core::entity::rules;
    use sea_orm::{ActiveModelTrait, Set};

    let rule = rules::ActiveModel {
        name: Set(req.name.clone()),
        script: Set(req.script.clone()),
        active: Set(true),
        created_at: Set(chrono::Utc::now().timestamp_millis()),
        ..Default::default()
    };

    let result = match rule.insert(&state.db).await {
        Ok(_) => {
            // 更新活跃规则数
            let rule_count = state.script_engine.get_script_ids().len();
            metrics::set_active_rules(rule_count);

            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "status": "created", "name": req.name })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to save rule to DB: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Database error: {}", e) })),
            )
        }
    };

    metrics::record_http_duration(start.elapsed().as_secs_f64());
    result
}

pub async fn reload_rules(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    tracing::info!("Reloading rules from Database...");

    use flux_core::entity::rules;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    // 1. Fetch active rules from DB
    let active_rules = match rules::Entity::find()
        .filter(rules::Column::Active.eq(true))
        .all(&state.db)
        .await
    {
        Ok(rules) => rules,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    // 2. Identify rules to compile and rules to remove
    let db_rule_names: Vec<String> = active_rules.iter().map(|r| r.name.clone()).collect();
    let cached_rule_names = state.script_engine.get_script_ids();

    // Compile/Update active rules
    for rule in active_rules {
        if let Err(e) = state.script_engine.compile_script(&rule.name, &rule.script) {
            tracing::error!("Failed to compile rule {}: {}", rule.name, e);
        } else {
            tracing::info!("Reloaded rule: {}", rule.name);
        }
    }

    // Remove rules that are no longer active or in DB
    for cached_name in cached_rule_names {
        if !db_rule_names.contains(&cached_name) {
            tracing::info!("Removing inactive rule: {}", cached_name);
            state.script_engine.remove_script(&cached_name);
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "reloaded", "count": db_rule_names.len() })),
    )
}

pub async fn list_rules(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let scripts = state.script_engine.get_script_ids();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "rules": scripts })),
    )
}

pub async fn get_storage_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let metrics = state.storage_manager.get_metrics().await;
    (StatusCode::OK, Json(metrics))
}

pub async fn get_storage_pools(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pools = state.storage_manager.get_pools_stats().await;
    (StatusCode::OK, Json(pools))
}

#[derive(Deserialize)]
pub struct StorageTelemetryRequest {
    pub topic: String,
    pub payload: Value,
}

pub async fn post_storage_telemetry(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StorageTelemetryRequest>,
) -> impl IntoResponse {
    metrics::record_http_request();
    let start = std::time::Instant::now();

    let service = req
        .payload
        .get("service")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    metrics::record_storage_telemetry(&req.topic, service.as_deref());

    let msg = Message::new(req.topic, req.payload);
    let msg_id = msg.id.to_string();

    if let Err(e) = state.event_bus.publish(msg) {
        tracing::warn!(
            "Telemetry published but no subscribers: {} (Error: {})",
            msg_id,
            e
        );
    } else {
        metrics::record_event_published();
    }

    metrics::record_http_duration(start.elapsed().as_secs_f64());

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "id": msg_id })),
    )
}

#[derive(Deserialize)]
pub struct StorageAuditQuery {
    pub topic_prefix: Option<String>,
    pub since: Option<i64>,
    pub limit: Option<u64>,
}

pub async fn get_storage_audit(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<StorageAuditQuery>,
) -> impl IntoResponse {
    let prefix = q.topic_prefix.unwrap_or_else(|| "storage/".to_string());
    let limit = q.limit.unwrap_or(200).min(1000);

    let mut stmt = events::Entity::find()
        .filter(events::Column::Topic.like(format!("{}%", prefix)))
        .order_by_desc(events::Column::Timestamp)
        .limit(limit);

    if let Some(since) = q.since {
        stmt = stmt.filter(events::Column::Timestamp.gte(since));
    }

    match stmt.all(&state.db).await {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(e) => {
            let msg = e.to_string();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response()
        }
    }
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/v1/storage/metrics", get(get_storage_metrics))
        .route("/api/v1/storage/pools", get(get_storage_pools))
        .route("/api/v1/storage/telemetry", post(post_storage_telemetry))
        .route("/api/v1/storage/audit", get(get_storage_audit))
        .route(
            "/api/v1/storage/telemetry/stats",
            get(get_storage_telemetry_stats),
        )
        .route(
            "/api/v1/storage/telemetry/troubleshoot",
            get(get_storage_telemetry_troubleshoot),
        )
        .route("/api/v1/event", post(accept_event))
        .route("/api/v1/rules", post(create_rule).get(list_rules))
        .route("/api/v1/rules/reload", post(reload_rules))
        .route("/api/v1/gb28181/invite", post(gb_invite))
        .route("/api/v1/gb28181/bye", post(gb_bye))
        .route("/api/v1/gb28181/catalog", post(gb_query_catalog))
        .route("/api/v1/gb28181/device-info", post(gb_query_device_info))
        .route("/api/v1/gb28181/device-status", post(gb_query_device_status))
        .route("/api/v1/gb28181/devices", get(gb_list_devices))
        .route("/api/v1/gb28181/devices/:device_id", get(gb_get_device))
        .route(
            "/api/v1/gb28181/devices/:device_id/channels",
            get(gb_list_device_channels),
        )
        .route(
            "/api/v1/gb28181/streams/:stream_id/snapshot",
            get(gb_snapshot),
        )
        .route(
            "/api/v1/app-config",
            get(get_app_config).post(update_app_config),
        )
        .route("/api/v1/app-config/audit", get(get_app_config_audit))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, body::Bytes, http::Request};
    use flux_server::gb28181_backend::{Gb28181BackendRef, RemoteBackend};
    use flux_core::bus::EventBus;
    use flux_plugin::manager::PluginManager;
    use flux_script::ScriptEngine;
    use hyper::body::to_bytes;
    use sea_orm::Database;
    use std::sync::Once;
    use tokio::sync::watch;
    use tokio::sync::oneshot;
    use tokio::net::UdpSocket;
    use flux_video::gb28181::sip::SipServer;
    use flux_video::gb28181::sip::SipServerConfig;
    use flux_video::gb28181::rtp::receiver::RtpReceiverConfig;
    use flux_video::gb28181::rtp::RtpReceiver;
    use flux_video::snapshot::KeyframeExtractor;
    use flux_video::storage::StandaloneStorage;
    use tokio::sync::RwLock;
    use tower::ServiceExt;

    static INIT: Once = Once::new();

    fn init_admin_token() {
        INIT.call_once(|| {
            std::env::set_var("FLUX_ADMIN_TOKEN", "test-token");
        });
    }

    async fn create_test_state_with_config_db() -> Arc<AppState> {
        let event_bus = Arc::new(EventBus::new(100));
        let plugin_manager = Arc::new(PluginManager::new().expect("plugin manager"));
        let script_engine = Arc::new(ScriptEngine::new());
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("db");
        let config_db = Database::connect("sqlite::memory:")
            .await
            .expect("config db");
        let (_tx, rx) = watch::channel(flux_server::AppConfig::default());

        let storage_manager = Arc::new(flux_storage::StorageManager::new());

        Arc::new(AppState {
            event_bus,
            plugin_manager,
            script_engine,
            storage_manager,
            db,
            config_db: Some(config_db),
            config: rx,
            gb28181_sip: None,
            gb28181_backend: None,
        })
    }

    pub(crate) async fn create_test_state_with_remote_backend(base_url: String) -> Arc<AppState> {
        let event_bus = Arc::new(EventBus::new(100));
        let plugin_manager = Arc::new(PluginManager::new().expect("plugin manager"));
        let script_engine = Arc::new(ScriptEngine::new());
        let db = Database::connect("sqlite::memory:").await.expect("db");

        let (_tx, rx) = watch::channel(flux_server::AppConfig::default());

        let storage_manager = Arc::new(flux_storage::StorageManager::new());

        let backend: Gb28181BackendRef = Arc::new(RemoteBackend::new(base_url));

        Arc::new(AppState {
            event_bus,
            plugin_manager,
            script_engine,
            storage_manager,
            db,
            config_db: None,
            config: rx,
            gb28181_sip: None,
            gb28181_backend: Some(backend),
        })
    }

    async fn spawn_mock_gb28181_http() -> (String, oneshot::Sender<()>) {
        async fn snapshot(Path(stream_id): Path<String>) -> impl IntoResponse {
            if stream_id == "missing" {
                return StatusCode::NOT_FOUND.into_response();
            }

            let mut resp: Response = Bytes::from_static(b"snapshot-bytes").into_response();
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            resp
        }

        async fn channels(Path(device_id): Path<String>) -> Response {
            if device_id == "missing" {
                return StatusCode::NOT_FOUND.into_response();
            }

            Json(serde_json::json!({
                "channels": [
                    {
                        "channel_id": "c1",
                        "name": "ch",
                        "status": "ON"
                    }
                ]
            }))
            .into_response()
        }

        let app = Router::new()
            .route(
                "/api/v1/gb28181/streams/:stream_id/snapshot",
                get(snapshot),
            )
            .route(
                "/api/v1/gb28181/devices/:device_id/channels",
                get(channels),
            );

        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = std_listener.local_addr().expect("addr");
        std_listener
            .set_nonblocking(true)
            .expect("set_nonblocking");

        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::Server::from_tcp(std_listener)
                .expect("from_tcp")
                .serve(app.into_make_service())
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                });

            let _ = server.await;
        });

        (format!("http://{}", addr), tx)
    }

    fn minimal_app_config_toml() -> String {
        r#"
[server]
host = "127.0.0.1"
port = 3000

[database]
url = "sqlite::memory:"

[plugins]
directory = "plugins"
"#
        .to_string()
    }

    fn minimal_app_config_toml_with_port(port: u16) -> String {
        format!(
            r#"
[server]
host = "127.0.0.1"
port = {port}

[database]
url = "sqlite::memory:"

[plugins]
directory = "plugins"
"#
        )
    }

    #[tokio::test]
    async fn test_app_config_unauthorized() {
        init_admin_token();
        let state = create_test_state_with_config_db().await;
        let app = create_router(state);

        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("GET")
            .body(Body::empty())
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_app_config_authorized_roundtrip() {
        init_admin_token();
        let state = create_test_state_with_config_db().await;
        let app = create_router(state);

        let body = serde_json::json!({ "content": minimal_app_config_toml() });
        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", "Bearer test-token")
            .body(Body::from(body.to_string()))
            .expect("request");

        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("GET")
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .expect("request");

        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .uri("/api/v1/app-config/audit")
            .method("GET")
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .expect("request");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_app_config_reject_blocked_fields() {
        init_admin_token();
        let state = create_test_state_with_config_db().await;
        let app = create_router(state);

        let body = serde_json::json!({ "content": minimal_app_config_toml() });
        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", "Bearer test-token")
            .body(Body::from(body.to_string()))
            .expect("request");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        // 修改 server.port（禁止热更新）应被拒绝
        let body = serde_json::json!({ "content": minimal_app_config_toml_with_port(3001) });
        let req = Request::builder()
            .uri("/api/v1/app-config")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", "Bearer test-token")
            .body(Body::from(body.to_string()))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_gb28181_remote_snapshot_success() {
        let (base_url, shutdown) = spawn_mock_gb28181_http().await;
        let state = create_test_state_with_remote_backend(base_url).await;
        let app = create_router(state);

        let req = Request::builder()
            .uri("/api/v1/gb28181/streams/s1/snapshot")
            .method("GET")
            .body(Body::empty())
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let body = to_bytes(resp.into_body()).await.expect("body");
        assert_eq!(&body[..], b"snapshot-bytes");

        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn test_gb28181_remote_snapshot_not_found() {
        let (base_url, shutdown) = spawn_mock_gb28181_http().await;
        let state = create_test_state_with_remote_backend(base_url).await;
        let app = create_router(state);

        let req = Request::builder()
            .uri("/api/v1/gb28181/streams/missing/snapshot")
            .method("GET")
            .body(Body::empty())
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn test_gb28181_remote_channels_shape() {
        let (base_url, shutdown) = spawn_mock_gb28181_http().await;
        let state = create_test_state_with_remote_backend(base_url).await;
        let app = create_router(state);

        let req = Request::builder()
            .uri("/api/v1/gb28181/devices/d1/channels")
            .method("GET")
            .body(Body::empty())
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let body = to_bytes(resp.into_body()).await.expect("body");
        let v: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert!(v.get("channels").is_some());

        let _ = shutdown.send(());
    }
}
