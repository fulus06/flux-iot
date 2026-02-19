use crate::error::Result;
use crate::types::StreamId;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// HLS 播放列表生成器
pub struct HlsGenerator {
    stream_id: StreamId,
    target_duration: u32,
    playlist_length: usize,
    segments: Arc<RwLock<VecDeque<HlsSegment>>>,
    sequence_number: Arc<RwLock<u64>>,
}

/// HLS 分片信息
#[derive(Debug, Clone)]
pub struct HlsSegment {
    pub sequence: u64,
    pub duration: f64,
    pub filename: String,
    pub timestamp: DateTime<Utc>,
    pub size: usize,
}

/// HLS 播放列表
#[derive(Debug, Clone)]
pub struct HlsPlaylist {
    pub version: u8,
    pub target_duration: u32,
    pub media_sequence: u64,
    pub segments: Vec<HlsSegment>,
}

impl HlsGenerator {
    pub fn new(stream_id: StreamId, target_duration: u32, playlist_length: usize) -> Self {
        Self {
            stream_id,
            target_duration,
            playlist_length,
            segments: Arc::new(RwLock::new(VecDeque::new())),
            sequence_number: Arc::new(RwLock::new(0)),
        }
    }

    /// 添加新的分片
    pub async fn add_segment(&self, duration: f64, size: usize) -> Result<HlsSegment> {
        let mut seq = self.sequence_number.write().await;
        let sequence = *seq;
        *seq += 1;

        let segment = HlsSegment {
            sequence,
            duration,
            filename: format!("segment_{}.ts", sequence),
            timestamp: Utc::now(),
            size,
        };

        let mut segments = self.segments.write().await;
        segments.push_back(segment.clone());

        // 保持播放列表长度
        while segments.len() > self.playlist_length {
            segments.pop_front();
        }

        Ok(segment)
    }

    /// 生成 M3U8 播放列表
    pub async fn generate_playlist(&self) -> Result<String> {
        let segments = self.segments.read().await;
        
        if segments.is_empty() {
            return Ok(self.generate_empty_playlist());
        }

        let media_sequence = segments.front().map(|s| s.sequence).unwrap_or(0);
        let mut m3u8 = String::new();

        // M3U8 头部
        m3u8.push_str("#EXTM3U\n");
        m3u8.push_str("#EXT-X-VERSION:3\n");
        m3u8.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", self.target_duration));
        m3u8.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{}\n", media_sequence));

        // 分片列表
        for segment in segments.iter() {
            m3u8.push_str(&format!("#EXTINF:{:.3},\n", segment.duration));
            m3u8.push_str(&format!("{}\n", segment.filename));
        }

        Ok(m3u8)
    }

    /// 生成空播放列表
    fn generate_empty_playlist(&self) -> String {
        let mut m3u8 = String::new();
        m3u8.push_str("#EXTM3U\n");
        m3u8.push_str("#EXT-X-VERSION:3\n");
        m3u8.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", self.target_duration));
        m3u8.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        m3u8
    }

    /// 获取指定分片
    pub async fn get_segment(&self, sequence: u64) -> Option<HlsSegment> {
        let segments = self.segments.read().await;
        segments.iter().find(|s| s.sequence == sequence).cloned()
    }

    /// 获取播放列表信息
    pub async fn get_playlist_info(&self) -> HlsPlaylist {
        let segments = self.segments.read().await;
        let media_sequence = segments.front().map(|s| s.sequence).unwrap_or(0);

        HlsPlaylist {
            version: 3,
            target_duration: self.target_duration,
            media_sequence,
            segments: segments.iter().cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hls_generator_creation() {
        let stream_id = StreamId::new("rtmp", "live/test");
        let generator = HlsGenerator::new(stream_id, 6, 5);
        
        let playlist = generator.generate_playlist().await.unwrap();
        assert!(playlist.contains("#EXTM3U"));
        assert!(playlist.contains("#EXT-X-VERSION:3"));
        assert!(playlist.contains("#EXT-X-TARGETDURATION:6"));
    }

    #[tokio::test]
    async fn test_add_segment() {
        let stream_id = StreamId::new("rtmp", "live/test");
        let generator = HlsGenerator::new(stream_id, 6, 5);

        let segment = generator.add_segment(6.0, 1024).await.unwrap();
        assert_eq!(segment.sequence, 0);
        assert_eq!(segment.duration, 6.0);
        assert_eq!(segment.filename, "segment_0.ts");

        let playlist = generator.generate_playlist().await.unwrap();
        assert!(playlist.contains("segment_0.ts"));
        assert!(playlist.contains("#EXTINF:6.000"));
    }

    #[tokio::test]
    async fn test_playlist_length_limit() {
        let stream_id = StreamId::new("rtmp", "live/test");
        let generator = HlsGenerator::new(stream_id, 6, 3);

        // 添加 5 个分片
        for i in 0..5 {
            generator.add_segment(6.0, 1024).await.unwrap();
        }

        let info = generator.get_playlist_info().await;
        assert_eq!(info.segments.len(), 3); // 只保留最后 3 个
        assert_eq!(info.segments[0].sequence, 2); // 从序号 2 开始
    }

    #[tokio::test]
    async fn test_get_segment() {
        let stream_id = StreamId::new("rtmp", "live/test");
        let generator = HlsGenerator::new(stream_id, 6, 5);

        generator.add_segment(6.0, 1024).await.unwrap();
        generator.add_segment(6.0, 2048).await.unwrap();

        let segment = generator.get_segment(1).await;
        assert!(segment.is_some());
        assert_eq!(segment.unwrap().size, 2048);

        let missing = generator.get_segment(999).await;
        assert!(missing.is_none());
    }
}
