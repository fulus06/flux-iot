// 关键帧提取模块（零解码方式）
use crate::codec::{H264Parser, H264Nalu};
use crate::Result;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[cfg(test)]
mod snapshot_test;

/// 关键帧提取器（零解码，仅解析 NALU）
pub struct KeyframeExtractor {
    /// H.264 解析器
    parser: H264Parser,
    
    /// 输出目录
    output_dir: PathBuf,
    
    /// 提取间隔（秒）
    interval_secs: u64,
    
    /// 上次提取时间
    last_extract_time: Option<DateTime<Utc>>,
}

impl KeyframeExtractor {
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            parser: H264Parser::new(),
            output_dir,
            interval_secs: 5, // 默认每 5 秒提取一次
            last_extract_time: None,
        }
    }
    
    /// 设置提取间隔
    pub fn with_interval(mut self, interval_secs: u64) -> Self {
        self.interval_secs = interval_secs;
        self
    }
    
    /// 处理视频数据，提取关键帧
    pub async fn process(
        &mut self,
        stream_id: &str,
        data: &[u8],
        timestamp: DateTime<Utc>,
    ) -> Result<Option<Keyframe>> {
        // 检查是否应该提取
        if !self.should_extract(timestamp) {
            return Ok(None);
        }
        
        // 解析 NALU
        let nalus = self.parser.parse_annexb(data);
        
        // 查找 IDR 帧
        let idr_nalu = nalus.iter().find(|n| n.is_keyframe());
        
        if let Some(idr) = idr_nalu {
            // 提取关键帧
            let keyframe = self.extract_keyframe(stream_id, idr, timestamp).await?;
            
            // 更新提取时间
            self.last_extract_time = Some(timestamp);
            
            tracing::debug!(
                "Extracted keyframe for stream {}: {} bytes",
                stream_id,
                keyframe.data.len()
            );
            
            return Ok(Some(keyframe));
        }
        
        Ok(None)
    }
    
    /// 判断是否应该提取
    fn should_extract(&self, timestamp: DateTime<Utc>) -> bool {
        if let Some(last_time) = self.last_extract_time {
            let elapsed = timestamp.signed_duration_since(last_time);
            elapsed.num_seconds() >= self.interval_secs as i64
        } else {
            true // 第一次提取
        }
    }
    
    /// 提取关键帧（零拷贝）
    async fn extract_keyframe(
        &self,
        stream_id: &str,
        idr_nalu: &H264Nalu,
        timestamp: DateTime<Utc>,
    ) -> Result<Keyframe> {
        // 构建完整的关键帧数据（SPS + PPS + IDR）
        let mut frame_data = Vec::new();
        
        // 添加 SPS
        if let Some(sps) = self.parser.sps() {
            frame_data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
            frame_data.extend_from_slice(sps);
        }
        
        // 添加 PPS
        if let Some(pps) = self.parser.pps() {
            frame_data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
            frame_data.extend_from_slice(pps);
        }
        
        // 添加 IDR
        frame_data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
        frame_data.extend_from_slice(&idr_nalu.data);
        
        // 生成文件路径
        let filename = format!(
            "{}_{}_{}.h264",
            stream_id,
            timestamp.timestamp(),
            timestamp.timestamp_subsec_millis()
        );
        
        let file_path = self.output_dir.join(stream_id).join(filename);
        
        // 确保目录存在
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // 保存到文件
        tokio::fs::write(&file_path, &frame_data).await?;
        
        Ok(Keyframe {
            stream_id: stream_id.to_string(),
            timestamp,
            data: Bytes::from(frame_data),
            file_path: file_path.to_string_lossy().to_string(),
        })
    }
    
    /// 获取解析器（用于检查参数集）
    pub fn parser(&self) -> &H264Parser {
        &self.parser
    }
}

impl Default for KeyframeExtractor {
    fn default() -> Self {
        Self::new(PathBuf::from("./keyframes"))
    }
}

/// 关键帧
#[derive(Debug, Clone)]
pub struct Keyframe {
    pub stream_id: String,
    pub timestamp: DateTime<Utc>,
    pub data: Bytes,
    pub file_path: String,
}

impl Keyframe {
    /// 获取文件大小
    pub fn size(&self) -> usize {
        self.data.len()
    }
}
