use crate::error::Result;
use crate::types::{AudioSample, StreamId, VideoSample};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 协议适配器 Trait（所有协议的统一接口）
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    /// 协议名称
    fn protocol_name(&self) -> &str;

    /// 启动协议服务
    async fn start(&self) -> Result<()>;

    /// 停止协议服务
    async fn stop(&self) -> Result<()>;

    /// 获取协议统计信息
    async fn stats(&self) -> ProtocolStats;
}

/// 流回调接口
#[async_trait]
pub trait StreamCallback: Send + Sync {
    /// 视频样本回调
    async fn on_video_sample(&self, stream_id: &StreamId, sample: VideoSample);

    /// 音频样本回调
    async fn on_audio_sample(&self, stream_id: &StreamId, sample: AudioSample);

    /// 流关闭回调
    async fn on_stream_closed(&self, stream_id: &StreamId);
}

/// 协议统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub protocol: String,
    pub active_streams: usize,
    pub total_bytes_received: u64,
    pub total_bytes_sent: u64,
    pub uptime_seconds: u64,
    pub custom_metrics: HashMap<String, f64>,
}

impl Default for ProtocolStats {
    fn default() -> Self {
        Self {
            protocol: String::new(),
            active_streams: 0,
            total_bytes_received: 0,
            total_bytes_sent: 0,
            uptime_seconds: 0,
            custom_metrics: HashMap::new(),
        }
    }
}

/// 流状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStatus {
    pub stream_id: StreamId,
    pub protocol: String,
    pub state: StreamState,
    pub bitrate_kbps: f64,
    pub fps: f64,
    pub resolution: Option<(u32, u32)>,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamState {
    Connecting,
    Active,
    Paused,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_stats_default() {
        let stats = ProtocolStats::default();
        assert_eq!(stats.active_streams, 0);
        assert_eq!(stats.total_bytes_received, 0);
    }

    #[test]
    fn test_stream_state() {
        let state = StreamState::Active;
        assert_eq!(state, StreamState::Active);
    }
}
