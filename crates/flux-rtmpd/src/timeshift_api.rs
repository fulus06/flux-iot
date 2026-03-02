use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::hls_manager::HlsManager;

/// 时移回放查询参数
#[derive(Debug, Deserialize)]
pub struct TimeshiftQuery {
    /// 开始时间（ISO 8601 格式）
    pub start_time: String,
    /// 持续时间（秒，可选）
    pub duration: Option<i64>,
    /// 是否从最近的关键帧开始
    pub from_keyframe: Option<bool>,
}

/// 时移回放 API 处理器
pub struct TimeshiftApi {
    hls_manager: Arc<HlsManager>,
}

impl TimeshiftApi {
    pub fn new(hls_manager: Arc<HlsManager>) -> Self {
        Self { hls_manager }
    }

    /// 获取时移回放播放列表
    pub async fn get_timeshift_playlist(
        hls_manager: Arc<HlsManager>,
        app_name: String,
        stream_key: String,
        query: TimeshiftQuery,
    ) -> Result<String, StatusCode> {
        let stream_id = format!("{}/{}", app_name, stream_key);
        
        info!(
            stream = %stream_id,
            start_time = %query.start_time,
            duration = ?query.duration,
            "Timeshift playback requested"
        );

        // 1. 解析开始时间
        let start_time = DateTime::parse_from_rfc3339(&query.start_time)
            .map_err(|e| {
                error!("Invalid start_time format: {}", e);
                StatusCode::BAD_REQUEST
            })?
            .with_timezone(&Utc);

        let end_time = query.duration.map(|d| {
            start_time + chrono::Duration::seconds(d)
        });

        // 2. 从 flux-storage 查询元数据
        let segment_storage = hls_manager.get_segment_storage();
        
        // 构建查询过滤器
        let mut filter = HashMap::new();
        filter.insert("protocol".to_string(), "hls".to_string());
        
        if query.from_keyframe.unwrap_or(false) {
            filter.insert("has_keyframe".to_string(), "true".to_string());
        }

        let segments = segment_storage
            .query_metadata(&stream_id, filter)
            .await
            .map_err(|e| {
                error!("Failed to query metadata: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // 3. 过滤时间范围
        let filtered_segments: Vec<_> = segments
            .into_iter()
            .filter(|(_, metadata)| {
                if let Some(start_time_str) = metadata.get("start_time") {
                    if let Ok(seg_time) = DateTime::parse_from_rfc3339(start_time_str) {
                        let seg_time = seg_time.with_timezone(&Utc);
                        
                        // 检查是否在时间范围内
                        if seg_time < start_time {
                            return false;
                        }
                        if let Some(end) = end_time {
                            if seg_time > end {
                                return false;
                            }
                        }
                        return true;
                    }
                }
                false
            })
            .collect();

        if filtered_segments.is_empty() {
            error!("No segments found for timeshift playback");
            return Err(StatusCode::NOT_FOUND);
        }

        info!(
            stream = %stream_id,
            segments_count = filtered_segments.len(),
            "Timeshift segments found"
        );

        // 4. 生成 M3U8 播放列表
        let playlist = generate_timeshift_m3u8(&stream_id, &filtered_segments)
            .map_err(|e| {
                error!("Failed to generate M3U8: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        Ok(playlist)
    }
}

/// 生成时移回放 M3U8 播放列表
fn generate_timeshift_m3u8(
    stream_id: &str,
    segments: &[(u64, flux_storage::SegmentMetadata)],
) -> Result<String> {
    let mut m3u8 = String::from("#EXTM3U\n");
    m3u8.push_str("#EXT-X-VERSION:3\n");
    m3u8.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n"); // VOD 类型（点播）
    
    // 计算最大时长
    let max_duration = segments
        .iter()
        .filter_map(|(_, meta)| meta.get("duration"))
        .filter_map(|d| d.parse::<f64>().ok())
        .fold(0.0, f64::max);
    
    m3u8.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", max_duration.ceil() as u64));
    m3u8.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
    
    // 添加分片
    for (segment_id, metadata) in segments {
        let duration = metadata
            .get("duration")
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(10.0);
        
        m3u8.push_str(&format!("#EXTINF:{:.3},\n", duration));
        m3u8.push_str(&format!("/hls/{}/segment_{}.ts\n", stream_id, segment_id));
    }
    
    m3u8.push_str("#EXT-X-ENDLIST\n");
    
    debug!(
        stream_id = %stream_id,
        segments_count = segments.len(),
        "M3U8 playlist generated"
    );
    
    Ok(m3u8)
}

// 注意：这个函数需要在 main.rs 中定义，因为它需要访问 AppState
// 这里只提供实现逻辑
