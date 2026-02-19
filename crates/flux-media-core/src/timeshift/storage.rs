use anyhow::Result;
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use std::collections::VecDeque;
use std::path::PathBuf;

use super::manager::{Segment, SegmentFormat};

/// 热缓存（内存）
pub struct HotBuffer {
    /// 流 ID
    pub stream_id: String,
    /// 分片列表（完整数据）
    pub segments: VecDeque<Segment>,
    /// 最大保留时长
    pub max_duration: Duration,
    /// 最后更新时间
    pub last_update: DateTime<Utc>,
}

impl HotBuffer {
    pub fn new(stream_id: &str, max_duration: Duration) -> Self {
        Self {
            stream_id: stream_id.to_string(),
            segments: VecDeque::new(),
            max_duration,
            last_update: Utc::now(),
        }
    }

    /// 添加分片
    pub fn add_segment(&mut self, segment: Segment) {
        self.segments.push_back(segment);
        self.last_update = Utc::now();
        self.cleanup();
    }

    /// 清理过期分片
    fn cleanup(&mut self) {
        let cutoff = Utc::now() - self.max_duration;
        
        while let Some(segment) = self.segments.front() {
            if segment.start_time < cutoff {
                self.segments.pop_front();
            } else {
                break;
            }
        }
    }

    /// 二分查找指定时间的分片索引
    pub fn binary_search_by_time(&self, target: DateTime<Utc>) -> usize {
        let mut left = 0;
        let mut right = self.segments.len();
        
        while left < right {
            let mid = (left + right) / 2;
            if self.segments[mid].start_time < target {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        
        left.saturating_sub(1)
    }

    /// 获取从指定时间开始的分片
    pub fn get_segments_from(&self, start_time: DateTime<Utc>) -> Vec<Segment> {
        let start_idx = self.binary_search_by_time(start_time);
        self.segments.iter()
            .skip(start_idx)
            .cloned()
            .collect()
    }

    /// 获取最新的 N 个分片
    pub fn get_latest(&self, count: usize) -> Vec<Segment> {
        self.segments.iter()
            .rev()
            .take(count)
            .rev()
            .cloned()
            .collect()
    }
}

/// 冷索引（磁盘）
pub struct ColdIndex {
    /// 流 ID
    pub stream_id: String,
    /// 分片元数据（不含数据）
    pub metadata: VecDeque<SegmentMeta>,
    /// 存储目录
    pub storage_dir: PathBuf,
    /// 最后更新时间
    pub last_update: DateTime<Utc>,
}

impl ColdIndex {
    pub fn new(stream_id: &str, storage_dir: PathBuf) -> Self {
        Self {
            stream_id: stream_id.to_string(),
            metadata: VecDeque::new(),
            storage_dir,
            last_update: Utc::now(),
        }
    }

    /// 添加元数据
    pub fn add_metadata(&mut self, meta: SegmentMeta) {
        self.metadata.push_back(meta);
        self.last_update = Utc::now();
    }

    /// 二分查找
    pub fn binary_search_by_time(&self, target: DateTime<Utc>) -> usize {
        let mut left = 0;
        let mut right = self.metadata.len();
        
        while left < right {
            let mid = (left + right) / 2;
            if self.metadata[mid].start_time < target {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        
        left.saturating_sub(1)
    }

    /// 清理过期元数据
    pub async fn cleanup(&mut self, max_duration: Duration) -> Result<()> {
        let cutoff = Utc::now() - max_duration;
        
        while let Some(meta) = self.metadata.front() {
            if meta.start_time < cutoff {
                // 删除文件
                if meta.file_path.exists() {
                    tokio::fs::remove_file(&meta.file_path).await?;
                }
                self.metadata.pop_front();
            } else {
                break;
            }
        }
        
        Ok(())
    }
}

/// 轻量级分片元数据
#[derive(Clone, Debug)]
pub struct SegmentMeta {
    pub sequence: u64,
    pub start_time: DateTime<Utc>,
    pub duration: f64,
    pub file_path: PathBuf,
    pub size: u64,
    pub format: SegmentFormat,
    pub has_keyframe: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hot_buffer_binary_search() {
        let mut buffer = HotBuffer::new("test", Duration::minutes(5));
        
        // 添加测试分片
        for i in 0..10 {
            let segment = Segment {
                sequence: i,
                start_time: Utc::now() + Duration::seconds(i as i64 * 6),
                duration: 6.0,
                data: Bytes::new(),
                metadata: super::super::manager::SegmentMetadata {
                    format: SegmentFormat::Ts,
                    has_keyframe: true,
                    file_path: None,
                    size: 0,
                },
            };
            buffer.add_segment(segment);
        }
        
        // 测试二分查找
        let target = Utc::now() + Duration::seconds(30);
        let idx = buffer.binary_search_by_time(target);
        assert!(idx < buffer.segments.len());
    }

    #[test]
    fn test_hot_buffer_get_latest() {
        let mut buffer = HotBuffer::new("test", Duration::minutes(5));
        
        for i in 0..10 {
            let segment = Segment {
                sequence: i,
                start_time: Utc::now(),
                duration: 6.0,
                data: Bytes::new(),
                metadata: super::super::manager::SegmentMetadata {
                    format: SegmentFormat::Ts,
                    has_keyframe: true,
                    file_path: None,
                    size: 0,
                },
            };
            buffer.add_segment(segment);
        }
        
        let latest = buffer.get_latest(5);
        assert_eq!(latest.len(), 5);
    }
}
