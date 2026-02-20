use anyhow::{anyhow, Result};
use flux_media_core::{
    snapshot::SnapshotOrchestrator,
    storage::filesystem::FileSystemStorage,
    timeshift::{TimeShiftCore, Segment, SegmentFormat, SegmentMetadata},
    types::StreamId,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

use crate::h264_depacketizer::{H264Depacketizer, H264Nalu};
use crate::rtcp_receiver::{RtcpReceiver, RtcpPacket};
use crate::rtp_receiver::RtpReceiver;
use crate::rtsp_client::RtspClient;
use crate::sdp_parser::SdpParser;
use crate::telemetry::TelemetryClient;

/// RTSP 流统计信息
#[derive(Debug, Clone, Default)]
pub struct RtspStreamStats {
    pub packets_received: u64,
    pub packets_lost: u32,
    pub fraction_lost: u8,
    pub jitter: u32,
    pub last_sr_timestamp: Option<u64>,
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
}

/// RTSP 流信息
#[derive(Debug, Clone)]
pub struct RtspStreamInfo {
    pub stream_id: StreamId,
    pub url: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub is_active: bool,
    pub frame_count: u64,
    pub last_keyframe_time: Option<chrono::DateTime<chrono::Utc>>,
    pub stats: RtspStreamStats,
}

/// RTSP 流管理器
pub struct RtspStreamManager {
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
    streams: Arc<RwLock<std::collections::HashMap<String, RtspStreamInfo>>>,
    timeshift: Option<Arc<TimeShiftCore>>,
    telemetry: TelemetryClient,
}

impl RtspStreamManager {
    pub fn new(
        storage: Arc<RwLock<FileSystemStorage>>,
        orchestrator: Arc<SnapshotOrchestrator>,
        timeshift: Option<Arc<TimeShiftCore>>,
        telemetry: TelemetryClient,
    ) -> Self {
        Self {
            storage,
            orchestrator,
            streams: Arc::new(RwLock::new(std::collections::HashMap::new())),
            timeshift,
            telemetry,
        }
    }

    /// 启动 RTSP 流
    pub async fn start_stream(&self, url: String) -> Result<()> {
        let stream_id = Self::url_to_stream_id(&url);
        
        // 检查是否已存在
        {
            let streams = self.streams.read().await;
            if streams.contains_key(&url) {
                return Err(anyhow!("Stream already exists: {}", url));
            }
        }
        
        // 注册流信息
        {
            let mut streams = self.streams.write().await;
            let info = RtspStreamInfo {
                stream_id: stream_id.clone(),
                url: url.to_string(),
                start_time: chrono::Utc::now(),
                is_active: true,
                frame_count: 0,
                last_keyframe_time: None,
                stats: RtspStreamStats::default(),
            };
            streams.insert(url.clone(), info);
        }
        
        info!(target: "rtsp_stream_manager", "Starting RTSP stream: {}", url);
        
        // 上报流启动事件
        if self.telemetry.enabled() {
            self.telemetry
                .post(
                    "stream/start",
                    serde_json::json!({
                        "service": "flux-rtspd",
                        "stream_id": stream_id.as_str(),
                        "url": url,
                    }),
                )
                .await;
        }
        
        // 启动流处理任务
        let storage = self.storage.clone();
        let orchestrator = self.orchestrator.clone();
        let streams = self.streams.clone();
        let timeshift = self.timeshift.clone();
        let telemetry = self.telemetry.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::stream_task(
                url.clone(),
                stream_id,
                storage,
                orchestrator,
                streams,
                timeshift,
                telemetry,
            ).await {
                error!(target: "rtsp_stream_manager", "Stream task failed: {}", e);
            }
        });
        
        Ok(())
    }

    /// 停止 RTSP 流
    pub async fn stop_stream(&self, url: &str) -> Result<()> {
        let stream_id = {
            let mut streams = self.streams.write().await;
            if let Some(info) = streams.get_mut(url) {
                info.is_active = false;
                info!(target: "rtsp_stream_manager", "Stopping RTSP stream: {}", url);
                Some(info.stream_id.clone())
            } else {
                return Err(anyhow!("Stream not found: {}", url));
            }
        };
        
        // 上报流停止事件
        if let Some(sid) = stream_id {
            if self.telemetry.enabled() {
                self.telemetry
                    .post(
                        "stream/stop",
                        serde_json::json!({
                            "service": "flux-rtspd",
                            "stream_id": sid.as_str(),
                            "url": url,
                        }),
                    )
                    .await;
            }
        }
        
        Ok(())
    }

    /// 获取流信息
    pub async fn get_stream_info(&self, url: &str) -> Option<RtspStreamInfo> {
        let streams = self.streams.read().await;
        streams.get(url).cloned()
    }

    /// 列出所有流
    pub async fn list_streams(&self) -> Vec<RtspStreamInfo> {
        let streams = self.streams.read().await;
        streams.values().cloned().collect()
    }

    /// 流处理任务
    async fn stream_task(
        url: String,
        stream_id: StreamId,
        storage: Arc<RwLock<FileSystemStorage>>,
        orchestrator: Arc<SnapshotOrchestrator>,
        streams: Arc<RwLock<std::collections::HashMap<String, RtspStreamInfo>>>,
        timeshift: Option<Arc<TimeShiftCore>>,
        telemetry: TelemetryClient,
    ) -> Result<()> {
        loop {
            // 检查流是否仍然活跃
            {
                let streams_lock = streams.read().await;
                if let Some(info) = streams_lock.get(&url) {
                    if !info.is_active {
                        info!(target: "rtsp_stream_manager", "Stream stopped: {}", url);
                        break;
                    }
                } else {
                    break;
                }
            }
            
            // 尝试连接和处理流
            match Self::process_stream(
                &url,
                &stream_id,
                &storage,
                &orchestrator,
                &streams,
                &timeshift,
                &telemetry,
            ).await {
                Ok(_) => {
                    info!(target: "rtsp_stream_manager", "Stream ended normally: {}", url);
                    break;
                }
                Err(e) => {
                    error!(target: "rtsp_stream_manager", "Stream error: {}, retrying in 5s", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
        
        // 清理流信息
        let mut streams_lock = streams.write().await;
        streams_lock.remove(&url);
        
        Ok(())
    }

    /// 处理流
    async fn process_stream(
        url: &str,
        stream_id: &StreamId,
        storage: &Arc<RwLock<FileSystemStorage>>,
        orchestrator: &Arc<SnapshotOrchestrator>,
        streams: &Arc<RwLock<std::collections::HashMap<String, RtspStreamInfo>>>,
        timeshift: &Option<Arc<TimeShiftCore>>,
        telemetry: &TelemetryClient,
    ) -> Result<()> {
        // 1. 创建 RTSP 客户端
        let mut client = RtspClient::new(url.to_string());
        client.connect().await?;
        
        // 2. OPTIONS
        client.options().await?;
        
        // 3. DESCRIBE (获取 SDP)
        let describe_resp = client.describe().await?;
        let sdp = SdpParser::parse(&describe_resp.body)?;
        
        // 4. 获取视频轨道
        let video_track = SdpParser::get_video_track(&sdp)
            .ok_or_else(|| anyhow!("No video track found"))?;
        
        // 5. SETUP (使用随机端口)
        let rtp_port = 5000 + (rand::random::<u16>() % 1000);
        let track_url = if let Some(control) = &video_track.control_url {
            if control.starts_with("rtsp://") {
                control.clone()
            } else {
                format!("{}/{}", url.trim_end_matches('/'), control)
            }
        } else {
            url.to_string()
        };
        
        client.setup(&track_url, rtp_port).await?;
        
        // 6. 启动 RTP 接收器
        let (receiver, mut rtp_rx) = RtpReceiver::new(rtp_port).await?;
        tokio::spawn(async move {
            receiver.start().await;
        });
        
        // 6.5. 启动 RTCP 接收器（RTP 端口 + 1）
        let rtcp_port = rtp_port + 1;
        let (rtcp_receiver, mut rtcp_rx) = RtcpReceiver::new(rtcp_port).await?;
        tokio::spawn(async move {
            rtcp_receiver.start().await;
        });
        
        // RTCP 统计处理任务
        let streams_for_stats = streams.clone();
        let url_for_stats = url.to_string();
        tokio::spawn(async move {
            while let Some(rtcp_packet) = rtcp_rx.recv().await {
                Self::process_rtcp_packet(&url_for_stats, rtcp_packet, &streams_for_stats).await;
            }
        });
        
        // 7. PLAY
        client.play().await?;
        
        info!(target: "rtsp_stream_manager", "RTSP stream playing: {}", url);
        
        // 8. 处理 RTP 数据
        let mut depacketizer = H264Depacketizer::new();
        let mut frame_count = 0u64;
        
        while let Some(rtp_packet) = rtp_rx.recv().await {
            // 检查流是否仍然活跃
            {
                let streams_lock = streams.read().await;
                if let Some(info) = streams_lock.get(url) {
                    if !info.is_active {
                        break;
                    }
                }
            }
            
            // 解包 H264
            match depacketizer.process_rtp(rtp_packet) {
                Ok(nalus) => {
                    for nalu in nalus {
                        frame_count += 1;
                        
                        // 保存到存储
                        if let Err(e) = Self::save_nalu(
                            stream_id,
                            &nalu,
                            storage,
                            orchestrator,
                            timeshift,
                            telemetry,
                        ).await {
                            if telemetry.enabled() {
                                telemetry
                                    .post(
                                        "storage/write_err",
                                        serde_json::json!({
                                            "service": "flux-rtspd",
                                            "stream_id": stream_id.as_str(),
                                            "error": e.to_string(),
                                        }),
                                    )
                                    .await;
                            }
                            return Err(e);
                        } else {
                            // 采样上报写入成功
                            if telemetry.enabled() {
                                telemetry
                                    .post_sampled(
                                        "storage/write_ok",
                                        serde_json::json!({
                                            "service": "flux-rtspd",
                                            "stream_id": stream_id.as_str(),
                                            "bytes": nalu.data.len(),
                                        }),
                                        200,
                                    )
                                    .await;
                            }
                        }
                        
                        // 更新流信息
                        if nalu.is_keyframe {
                            let mut streams_lock = streams.write().await;
                            if let Some(info) = streams_lock.get_mut(url) {
                                info.frame_count = frame_count;
                                info.last_keyframe_time = Some(chrono::Utc::now());
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(target: "rtsp_stream_manager", "Failed to depacketize RTP: {}", e);
                }
            }
        }
        
        // 9. TEARDOWN
        let _ = client.teardown().await;
        
        Ok(())
    }

    /// 处理 RTCP 包
    async fn process_rtcp_packet(
        url: &str,
        packet: RtcpPacket,
        streams: &Arc<RwLock<std::collections::HashMap<String, RtspStreamInfo>>>,
    ) {
        let mut streams_lock = streams.write().await;
        if let Some(info) = streams_lock.get_mut(url) {
            match packet {
                RtcpPacket::SenderReport(sr) => {
                    info.stats.last_sr_timestamp = Some(sr.ntp_timestamp);
                    info.stats.packets_received = sr.packet_count as u64;
                    info.stats.last_update = Some(chrono::Utc::now());
                    
                    debug!(target: "rtsp_stream_manager",
                        "SR: packets={}, octets={}, rtp_ts={}",
                        sr.packet_count, sr.octet_count, sr.rtp_timestamp
                    );
                }
                RtcpPacket::ReceiverReport(rr) => {
                    if let Some(block) = rr.report_blocks.first() {
                        info.stats.fraction_lost = block.fraction_lost;
                        info.stats.packets_lost = block.cumulative_lost;
                        info.stats.jitter = block.jitter;
                        info.stats.last_update = Some(chrono::Utc::now());
                        
                        debug!(target: "rtsp_stream_manager",
                            "RR: lost={}/{}, jitter={}",
                            block.fraction_lost, block.cumulative_lost, block.jitter
                        );
                    }
                }
                RtcpPacket::Unknown(_) => {}
            }
        }
    }

    /// 保存 NALU 到存储
    async fn save_nalu(
        stream_id: &StreamId,
        nalu: &H264Nalu,
        _storage: &Arc<RwLock<FileSystemStorage>>,
        orchestrator: &Arc<SnapshotOrchestrator>,
        timeshift: &Option<Arc<TimeShiftCore>>,
        _telemetry: &TelemetryClient,
    ) -> Result<()> {
        use bytes::Bytes;
        use chrono::Utc;
        
        // 添加到时移
        if let Some(ref ts) = timeshift {
            let segment = Segment {
                sequence: nalu.timestamp as u64,
                start_time: Utc::now(),
                duration: 0.04,  // 假设 25fps
                data: nalu.data.clone(),
                metadata: SegmentMetadata {
                    format: SegmentFormat::Raw,
                    has_keyframe: nalu.is_keyframe,
                    file_path: None,
                    size: nalu.data.len() as u64,
                },
            };
            
            if let Err(e) = ts.add_segment(stream_id.as_str(), segment).await {
                warn!(target: "rtsp_stream_manager", "Failed to add segment to timeshift: {}", e);
            }
        }
        
        // 如果是关键帧，提取 snapshot
        if nalu.is_keyframe {
            debug!(target: "rtsp_stream_manager", 
                "Keyframe detected for {}, size={}", 
                stream_id.as_str(), 
                nalu.data.len()
            );
            
            // 处理关键帧用于 snapshot
            let timestamp = Utc::now();
            if let Err(e) = orchestrator.process_keyframe(stream_id, &nalu.data, timestamp).await {
                warn!(target: "rtsp_stream_manager", "Failed to process keyframe: {}", e);
            }
        }
        
        Ok(())
    }

    /// URL 转换为 StreamId
    fn url_to_stream_id(url: &str) -> StreamId {
        // rtsp://192.168.1.100:554/stream1 -> rtsp/192.168.1.100:554/stream1
        let path = url.strip_prefix("rtsp://").unwrap_or(url);
        StreamId::new("rtsp", path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_to_stream_id() {
        let stream_id = RtspStreamManager::url_to_stream_id("rtsp://192.168.1.100:554/stream1");
        assert_eq!(stream_id.as_str(), "rtsp/192.168.1.100:554/stream1");
    }

    #[test]
    fn test_stream_info_creation() {
        let info = RtspStreamInfo {
            stream_id: StreamId::new("rtsp", "192.168.1.100:554/stream1"),
            url: "rtsp://192.168.1.100:554/stream1".to_string(),
            start_time: chrono::Utc::now(),
            is_active: true,
            frame_count: 0,
            last_keyframe_time: None,
            stats: RtspStreamStats::default(),
        };
        
        assert!(info.is_active);
        assert_eq!(info.frame_count, 0);
    }
}
