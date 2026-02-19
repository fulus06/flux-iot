use anyhow::Result;
use bytes::Bytes;
use flux_media_core::{
    snapshot::SnapshotOrchestrator,
    storage::{filesystem::FileSystemStorage, MediaStorage},
    types::{AudioCodec, AudioSample, StreamId, VideoCodec, VideoSample},
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// 媒体处理器：负责处理 RTMP 推流的音视频数据
pub struct MediaProcessor {
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
}

impl MediaProcessor {
    pub fn new(
        storage: Arc<RwLock<FileSystemStorage>>,
        orchestrator: Arc<SnapshotOrchestrator>,
    ) -> Self {
        Self {
            storage,
            orchestrator,
        }
    }

    /// 处理视频数据
    pub async fn process_video(
        &self,
        stream_id: &StreamId,
        data: &[u8],
        timestamp_ms: u32,
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        // 解析 FLV 视频标签
        let video_info = self.parse_flv_video_tag(data)?;

        debug!(
            target: "rtmpd",
            stream_id = %stream_id,
            codec = ?video_info.codec,
            is_keyframe = video_info.is_keyframe,
            size = data.len(),
            "Processing video data"
        );

        // 创建视频样本
        let sample = VideoSample {
            data: Bytes::copy_from_slice(&video_info.payload),
            timestamp: chrono::Utc::now(),
            pts: Some(timestamp_ms as i64),
            dts: None,
            is_keyframe: video_info.is_keyframe,
            codec: video_info.codec,
        };

        // 存储视频数据
        let mut storage = self.storage.write().await;
        storage
            .put_object(stream_id, sample.timestamp, sample.data.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Storage error: {}", e))?;

        drop(storage);

        // 如果是关键帧，提取 snapshot
        if video_info.is_keyframe {
            if let Err(e) = self
                .orchestrator
                .process_keyframe(stream_id, &video_info.payload, sample.timestamp)
                .await
            {
                error!(target: "rtmpd", stream_id = %stream_id, "Keyframe extraction failed: {}", e);
            } else {
                info!(target: "rtmpd", stream_id = %stream_id, "Keyframe extracted");
            }
        }

        Ok(())
    }

    /// 处理音频数据
    pub async fn process_audio(
        &self,
        stream_id: &StreamId,
        data: &[u8],
        timestamp_ms: u32,
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        // 解析 FLV 音频标签
        let audio_info = self.parse_flv_audio_tag(data)?;

        debug!(
            target: "rtmpd",
            stream_id = %stream_id,
            codec = ?audio_info.codec,
            size = data.len(),
            "Processing audio data"
        );

        // 创建音频样本
        let sample = AudioSample {
            data: Bytes::copy_from_slice(&audio_info.payload),
            timestamp: chrono::Utc::now(),
            pts: Some(timestamp_ms as i64),
            codec: audio_info.codec,
            sample_rate: audio_info.sample_rate,
            channels: audio_info.channels,
        };

        // 存储音频数据（可选）
        // 目前只存储视频，音频可以后续扩展
        drop(sample);

        Ok(())
    }

    /// 解析 FLV 视频标签
    fn parse_flv_video_tag(&self, data: &[u8]) -> Result<VideoInfo> {
        if data.is_empty() {
            return Err(anyhow::anyhow!("Empty video data"));
        }

        let first_byte = data[0];
        let frame_type = (first_byte >> 4) & 0x0F;
        let codec_id = first_byte & 0x0F;

        // 判断是否为关键帧
        let is_keyframe = frame_type == 1; // 1 = keyframe, 2 = inter frame

        // 解析编码格式
        let codec = match codec_id {
            7 => VideoCodec::H264,  // AVC
            12 => VideoCodec::H265, // HEVC
            _ => VideoCodec::Unknown,
        };

        // 对于 H264，需要跳过 AVC packet header
        let payload = if codec == VideoCodec::H264 && data.len() > 5 {
            let avc_packet_type = data[1];
            if avc_packet_type == 0 {
                // AVC sequence header (SPS/PPS)
                &data[5..]
            } else if avc_packet_type == 1 {
                // AVC NALU
                &data[5..]
            } else {
                // AVC end of sequence
                &data[1..]
            }
        } else {
            &data[1..]
        };

        Ok(VideoInfo {
            codec,
            is_keyframe,
            payload: payload.to_vec(),
        })
    }

    /// 解析 FLV 音频标签
    fn parse_flv_audio_tag(&self, data: &[u8]) -> Result<AudioInfo> {
        if data.is_empty() {
            return Err(anyhow::anyhow!("Empty audio data"));
        }

        let first_byte = data[0];
        let sound_format = (first_byte >> 4) & 0x0F;
        let sound_rate = (first_byte >> 2) & 0x03;
        let sound_size = (first_byte >> 1) & 0x01;
        let sound_type = first_byte & 0x01;

        // 解析编码格式
        let codec = match sound_format {
            2 => AudioCodec::MP3,
            10 => AudioCodec::AAC,
            _ => AudioCodec::Unknown,
        };

        // 解析采样率
        let sample_rate = match sound_rate {
            0 => 5512,
            1 => 11025,
            2 => 22050,
            3 => 44100,
            _ => 44100,
        };

        // 解析声道数
        let channels = if sound_type == 0 { 1 } else { 2 };

        // 对于 AAC，需要跳过 AAC packet header
        let payload = if codec == AudioCodec::AAC && data.len() > 2 {
            let aac_packet_type = data[1];
            if aac_packet_type == 0 {
                // AAC sequence header
                &data[2..]
            } else {
                // AAC raw
                &data[2..]
            }
        } else {
            &data[1..]
        };

        Ok(AudioInfo {
            codec,
            sample_rate,
            channels,
            payload: payload.to_vec(),
        })
    }
}

struct VideoInfo {
    codec: VideoCodec,
    is_keyframe: bool,
    payload: Vec<u8>,
}

struct AudioInfo {
    codec: AudioCodec,
    sample_rate: u32,
    channels: u8,
    payload: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_media_core::storage::StorageConfig;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_media_processor_creation() {
        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            root_dir: temp_dir.path().to_path_buf(),
            retention_days: 7,
            segment_duration_secs: 60,
        };
        let storage = Arc::new(RwLock::new(FileSystemStorage::new(config).unwrap()));
        let orchestrator = Arc::new(SnapshotOrchestrator::new(temp_dir.path().to_path_buf()));

        let processor = MediaProcessor::new(storage, orchestrator);
        assert!(true); // 确保能够创建
    }

    #[test]
    fn test_parse_h264_keyframe() {
        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            root_dir: temp_dir.path().to_path_buf(),
            retention_days: 7,
            segment_duration_secs: 60,
        };
        let storage = Arc::new(RwLock::new(FileSystemStorage::new(config).unwrap()));
        let orchestrator = Arc::new(SnapshotOrchestrator::new(temp_dir.path().to_path_buf()));
        let processor = MediaProcessor::new(storage, orchestrator);

        // 模拟 H264 关键帧数据
        // Frame type = 1 (keyframe), Codec ID = 7 (AVC)
        let mut data = vec![0x17]; // 0001 0111
        data.push(1); // AVC NALU
        data.extend_from_slice(&[0, 0, 0, 0]); // composition time
        data.extend_from_slice(&[0, 0, 0, 1, 0x65]); // NALU start code + IDR

        let result = processor.parse_flv_video_tag(&data).unwrap();
        assert_eq!(result.codec, VideoCodec::H264);
        assert!(result.is_keyframe);
    }

    #[test]
    fn test_parse_aac_audio() {
        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            root_dir: temp_dir.path().to_path_buf(),
            retention_days: 7,
            segment_duration_secs: 60,
        };
        let storage = Arc::new(RwLock::new(FileSystemStorage::new(config).unwrap()));
        let orchestrator = Arc::new(SnapshotOrchestrator::new(temp_dir.path().to_path_buf()));
        let processor = MediaProcessor::new(storage, orchestrator);

        // 模拟 AAC 音频数据
        // Sound format = 10 (AAC), Rate = 3 (44kHz), Size = 1 (16-bit), Type = 1 (stereo)
        let mut data = vec![0xAF]; // 1010 1111
        data.push(1); // AAC raw
        data.extend_from_slice(&[0x12, 0x34]); // AAC data

        let result = processor.parse_flv_audio_tag(&data).unwrap();
        assert_eq!(result.codec, AudioCodec::AAC);
        assert_eq!(result.sample_rate, 44100);
        assert_eq!(result.channels, 2);
    }
}
