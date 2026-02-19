use anyhow::{anyhow, Result};
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use super::config::TimeShiftConfig;
use super::storage::{ColdIndex, HotBuffer, SegmentMeta};

/// 分片格式
#[derive(Clone, Debug, PartialEq)]
pub enum SegmentFormat {
    Ts,   // MPEG-TS (用于 HLS)
    Flv,  // FLV (用于 HTTP-FLV)
    Raw,  // 原始数据
}

/// 分片元数据
#[derive(Clone, Debug)]
pub struct SegmentMetadata {
    pub format: SegmentFormat,
    pub has_keyframe: bool,
    pub file_path: Option<PathBuf>,
    pub size: u64,
}

/// 通用分片
#[derive(Clone, Debug)]
pub struct Segment {
    pub sequence: u64,
    pub start_time: DateTime<Utc>,
    pub duration: f64,
    pub data: Bytes,
    pub metadata: SegmentMetadata,
}

/// 时移核心管理器
pub struct TimeShiftCore {
    /// 热缓存（内存）
    hot_cache: Arc<RwLock<HashMap<String, HotBuffer>>>,
    
    /// 冷索引（磁盘）
    cold_index: Arc<RwLock<HashMap<String, ColdIndex>>>,
    
    /// 配置
    config: TimeShiftConfig,
    
    /// 存储根目录
    storage_root: PathBuf,
}

impl TimeShiftCore {
    pub fn new(config: TimeShiftConfig, storage_root: PathBuf) -> Self {
        let core = Self {
            hot_cache: Arc::new(RwLock::new(HashMap::new())),
            cold_index: Arc::new(RwLock::new(HashMap::new())),
            config,
            storage_root,
        };
        
        // 启动后台清理任务
        core.start_cleanup_task();
        
        core
    }

    /// 添加分片
    pub async fn add_segment(&self, stream_id: &str, segment: Segment) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // 1. 添加到热缓存
        {
            let mut hot = self.hot_cache.write().await;
            let buffer = hot.entry(stream_id.to_string())
                .or_insert_with(|| {
                    HotBuffer::new(
                        stream_id,
                        Duration::seconds(self.config.hot_cache_duration as i64),
                    )
                });
            
            buffer.add_segment(segment.clone());
        }

        // 2. 异步保存到磁盘
        if segment.metadata.format != SegmentFormat::Raw {
            let stream_id = stream_id.to_string();
            let segment_clone = segment.clone();
            let storage_root = self.storage_root.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::save_to_disk(&storage_root, &stream_id, &segment_clone).await {
                    error!(target: "timeshift", "Failed to save segment: {}", e);
                }
            });
        }

        Ok(())
    }

    /// 获取从指定时间开始的分片
    pub async fn get_segments_from(
        &self,
        stream_id: &str,
        start_time: DateTime<Utc>,
        format: Option<SegmentFormat>,
    ) -> Result<Vec<Segment>> {
        let now = Utc::now();
        let offset = now - start_time;
        
        if offset <= Duration::seconds(self.config.hot_cache_duration as i64) {
            // 从热缓存读取
            self.get_from_hot(stream_id, start_time, format).await
        } else {
            // 从冷索引读取
            self.get_from_cold(stream_id, start_time, format).await
        }
    }

    /// 获取最新的 N 个分片
    pub async fn get_latest_segments(
        &self,
        stream_id: &str,
        count: usize,
        format: Option<SegmentFormat>,
    ) -> Result<Vec<Segment>> {
        let hot = self.hot_cache.read().await;
        let buffer = hot.get(stream_id)
            .ok_or_else(|| anyhow!("Stream not found"))?;
        
        let segments = buffer.get_latest(count);
        
        // 过滤格式
        if let Some(fmt) = format {
            Ok(segments.into_iter()
                .filter(|s| s.metadata.format == fmt)
                .collect())
        } else {
            Ok(segments)
        }
    }

    /// 从热缓存读取
    async fn get_from_hot(
        &self,
        stream_id: &str,
        start_time: DateTime<Utc>,
        format: Option<SegmentFormat>,
    ) -> Result<Vec<Segment>> {
        let hot = self.hot_cache.read().await;
        let buffer = hot.get(stream_id)
            .ok_or_else(|| anyhow!("Stream not found"))?;
        
        let segments = buffer.get_segments_from(start_time);
        
        // 过滤格式
        if let Some(fmt) = format {
            Ok(segments.into_iter()
                .filter(|s| s.metadata.format == fmt)
                .collect())
        } else {
            Ok(segments)
        }
    }

    /// 从冷索引读取
    async fn get_from_cold(
        &self,
        stream_id: &str,
        start_time: DateTime<Utc>,
        format: Option<SegmentFormat>,
    ) -> Result<Vec<Segment>> {
        let cold = self.cold_index.read().await;
        let index = cold.get(stream_id)
            .ok_or_else(|| anyhow!("Stream not found in cold storage"))?;
        
        let start_idx = index.binary_search_by_time(start_time);
        
        // 读取最多 10 个分片
        let metas: Vec<_> = index.metadata.iter()
            .skip(start_idx)
            .take(10)
            .filter(|m| {
                format.as_ref()
                    .map(|f| &m.format == f)
                    .unwrap_or(true)
            })
            .collect();
        
        // 并行读取文件
        let mut segments = Vec::new();
        for meta in metas {
            match tokio::fs::read(&meta.file_path).await {
                Ok(data) => {
                    segments.push(Segment {
                        sequence: meta.sequence,
                        start_time: meta.start_time,
                        duration: meta.duration,
                        data: Bytes::from(data),
                        metadata: SegmentMetadata {
                            format: meta.format.clone(),
                            has_keyframe: meta.has_keyframe,
                            file_path: Some(meta.file_path.clone()),
                            size: meta.size,
                        },
                    });
                }
                Err(e) => {
                    error!(target: "timeshift", "Failed to read segment file: {}", e);
                }
            }
        }
        
        Ok(segments)
    }

    /// 保存分片到磁盘
    async fn save_to_disk(
        storage_root: &PathBuf,
        stream_id: &str,
        segment: &Segment,
    ) -> Result<()> {
        let stream_dir = storage_root.join(stream_id);
        tokio::fs::create_dir_all(&stream_dir).await?;
        
        let filename = format!("segment_{}_{}.dat", 
            segment.start_time.timestamp(),
            segment.sequence
        );
        let file_path = stream_dir.join(&filename);
        
        tokio::fs::write(&file_path, &segment.data).await?;
        
        debug!(target: "timeshift",
            stream_id = %stream_id,
            sequence = segment.sequence,
            size = segment.data.len(),
            "Segment saved to disk"
        );
        
        Ok(())
    }

    /// 启动后台清理任务
    fn start_cleanup_task(&self) {
        let hot_cache = self.hot_cache.clone();
        let cold_index = self.cold_index.clone();
        let cold_duration = Duration::seconds(self.config.cold_storage_duration as i64);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(60)
            );
            
            loop {
                interval.tick().await;
                
                // 清理冷索引
                let mut cold = cold_index.write().await;
                for (stream_id, index) in cold.iter_mut() {
                    if let Err(e) = index.cleanup(cold_duration).await {
                        error!(target: "timeshift",
                            stream_id = %stream_id,
                            "Cleanup failed: {}", e
                        );
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_timeshift_core_add_segment() {
        let temp_dir = tempdir().unwrap();
        let config = TimeShiftConfig::default();
        let core = TimeShiftCore::new(config, temp_dir.path().to_path_buf());
        
        let segment = Segment {
            sequence: 0,
            start_time: Utc::now(),
            duration: 6.0,
            data: Bytes::from("test data"),
            metadata: SegmentMetadata {
                format: SegmentFormat::Ts,
                has_keyframe: true,
                file_path: None,
                size: 9,
            },
        };
        
        let result = core.add_segment("test_stream", segment).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_timeshift_core_get_latest() {
        let temp_dir = tempdir().unwrap();
        let config = TimeShiftConfig::default();
        let core = TimeShiftCore::new(config, temp_dir.path().to_path_buf());
        
        // 添加多个分片
        for i in 0..10 {
            let segment = Segment {
                sequence: i,
                start_time: Utc::now(),
                duration: 6.0,
                data: Bytes::from(format!("test data {}", i)),
                metadata: SegmentMetadata {
                    format: SegmentFormat::Ts,
                    has_keyframe: true,
                    file_path: None,
                    size: 10,
                },
            };
            core.add_segment("test_stream", segment).await.unwrap();
        }
        
        // 获取最新 5 个
        let segments = core.get_latest_segments("test_stream", 5, None).await.unwrap();
        assert_eq!(segments.len(), 5);
    }
}
