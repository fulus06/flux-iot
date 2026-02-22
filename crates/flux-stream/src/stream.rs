use async_trait::async_trait;
use bytes::Bytes;
use flux_media_core::types::StreamId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    RTMP,
    RTSP,
    SRT,
    WebRTC,
    HttpFlv,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::RTMP => write!(f, "rtmp"),
            Protocol::RTSP => write!(f, "rtsp"),
            Protocol::SRT => write!(f, "srt"),
            Protocol::WebRTC => write!(f, "webrtc"),
            Protocol::HttpFlv => write!(f, "http-flv"),
        }
    }
}

/// 流元数据
#[derive(Debug, Clone)]
pub struct StreamMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub framerate: Option<f32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub bitrate: Option<u32>,
}

impl Default for StreamMetadata {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            framerate: None,
            video_codec: None,
            audio_codec: None,
            bitrate: None,
        }
    }
}

/// 流状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStatus {
    Idle,
    Starting,
    Running,
    Paused,
    Stopping,
    Stopped,
    Error,
}

/// 流抽象 trait（所有协议流都实现这个）
#[async_trait]
pub trait Stream: Send + Sync {
    fn stream_id(&self) -> &StreamId;
    fn protocol(&self) -> Protocol;
    
    async fn metadata(&self) -> StreamMetadata;
    async fn status(&self) -> StreamStatus;
    
    async fn start(&mut self) -> anyhow::Result<()>;
    async fn stop(&mut self) -> anyhow::Result<()>;
}

/// 媒体数据包
#[derive(Debug, Clone)]
pub struct MediaPacket {
    pub data: Bytes,
    pub timestamp: u32,
    pub is_keyframe: bool,
    pub packet_type: PacketType,
}

/// 数据包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Video,
    Audio,
}

/// 客户端信息
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub client_id: String,
    pub client_type: ClientType,
    pub preferred_protocol: Protocol,
    pub bandwidth_estimate: Option<u32>,
    pub user_agent: Option<String>,
}

/// 客户端类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientType {
    WebBrowser,
    MobileApp,
    Desktop,
    IoTDevice,
    Unknown,
}

/// 输出流
#[derive(Debug)]
pub struct OutputStream {
    pub stream_id: StreamId,
    pub protocol: Protocol,
    pub url: String,
    pub quality: QualityLevel,
}

/// 质量级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityLevel {
    Auto,
    High,
    Medium,
    Low,
}
