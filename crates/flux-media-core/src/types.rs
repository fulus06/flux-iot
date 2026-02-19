use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// 协议无关的流 ID
/// 格式：{protocol}/{identifier}
/// 例如：gb28181/34020000001320000001/34020000001320000001
///      rtmp/live/stream123
///      rtsp/192.168.1.100/channel1
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StreamId(String);

impl StreamId {
    pub fn new(protocol: &str, identifier: &str) -> Self {
        Self(format!("{}/{}", protocol, identifier))
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn protocol(&self) -> Option<&str> {
        self.0.split('/').next()
    }

    pub fn identifier(&self) -> Option<&str> {
        self.0.split_once('/').map(|(_, id)| id)
    }
}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for StreamId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for StreamId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// 视频样本（协议无关）
#[derive(Debug, Clone)]
pub struct VideoSample {
    pub data: Bytes,
    pub timestamp: DateTime<Utc>,
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub is_keyframe: bool,
    pub codec: VideoCodec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    Unknown,
}

/// 关键帧信息
#[derive(Debug, Clone)]
pub struct KeyframeInfo {
    pub stream_id: StreamId,
    pub file_path: String,
    pub timestamp: DateTime<Utc>,
    pub size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_id_gb28181() {
        let id = StreamId::new("gb28181", "34020000001320000001/34020000001320000001");
        assert_eq!(id.protocol(), Some("gb28181"));
        assert_eq!(
            id.identifier(),
            Some("34020000001320000001/34020000001320000001")
        );
        assert_eq!(
            id.as_str(),
            "gb28181/34020000001320000001/34020000001320000001"
        );
    }

    #[test]
    fn test_stream_id_rtmp() {
        let id = StreamId::new("rtmp", "live/stream123");
        assert_eq!(id.protocol(), Some("rtmp"));
        assert_eq!(id.identifier(), Some("live/stream123"));
        assert_eq!(id.as_str(), "rtmp/live/stream123");
    }
}
