// 监控指标模块

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// 视频质量监控器
pub struct QualityMonitor {
    /// 帧时间戳队列（用于计算 FPS）
    frame_timestamps: VecDeque<Instant>,
    /// 帧大小队列（用于计算比特率）
    frame_sizes: VecDeque<usize>,
    /// 统计窗口大小（秒）
    window_size: Duration,
    /// 总接收字节数
    total_bytes: u64,
    /// 总帧数
    total_frames: u64,
    /// 丢帧数
    dropped_frames: u64,
    /// 上一帧时间
    last_frame_time: Option<Instant>,
}

impl QualityMonitor {
    /// 创建新的质量监控器
    pub fn new() -> Self {
        Self::with_window_size(Duration::from_secs(5))
    }

    /// 创建指定窗口大小的监控器
    pub fn with_window_size(window_size: Duration) -> Self {
        Self {
            frame_timestamps: VecDeque::new(),
            frame_sizes: VecDeque::new(),
            window_size,
            total_bytes: 0,
            total_frames: 0,
            dropped_frames: 0,
            last_frame_time: None,
        }
    }

    /// 记录新帧
    pub fn record_frame(&mut self, frame_size: usize) {
        let now = Instant::now();
        
        // 检测丢帧（如果帧间隔过大）
        if let Some(last_time) = self.last_frame_time {
            let interval = now.duration_since(last_time);
            // 如果间隔超过 100ms（假设 10fps 最低），认为可能丢帧
            if interval > Duration::from_millis(100) {
                let expected_frames = (interval.as_millis() / 33) as u64; // 假设 30fps
                if expected_frames > 1 {
                    self.dropped_frames += expected_frames - 1;
                }
            }
        }
        
        self.last_frame_time = Some(now);
        self.total_frames += 1;
        self.total_bytes += frame_size as u64;
        
        // 添加到队列
        self.frame_timestamps.push_back(now);
        self.frame_sizes.push_back(frame_size);
        
        // 清理过期数据
        self.cleanup_old_data(now);
    }

    /// 清理超出窗口的旧数据
    fn cleanup_old_data(&mut self, now: Instant) {
        let cutoff = now - self.window_size;
        
        while let Some(&timestamp) = self.frame_timestamps.front() {
            if timestamp < cutoff {
                self.frame_timestamps.pop_front();
                self.frame_sizes.pop_front();
            } else {
                break;
            }
        }
    }

    /// 计算当前质量指标
    pub fn calculate_metrics(&self) -> QualityMetrics {
        let fps = self.calculate_fps();
        let bitrate = self.calculate_bitrate();
        let quality_score = self.calculate_quality_score(fps, bitrate);
        
        QualityMetrics {
            bitrate,
            fps,
            resolution: (0, 0), // 需要从帧数据中解析
            quality_score,
            total_frames: self.total_frames,
            dropped_frames: self.dropped_frames,
            drop_rate: if self.total_frames > 0 {
                (self.dropped_frames as f32 / self.total_frames as f32) * 100.0
            } else {
                0.0
            },
        }
    }

    /// 计算 FPS
    fn calculate_fps(&self) -> f32 {
        if self.frame_timestamps.len() < 2 {
            return 0.0;
        }
        
        let first = self.frame_timestamps.front().unwrap();
        let last = self.frame_timestamps.back().unwrap();
        let duration = last.duration_since(*first);
        
        if duration.as_secs_f32() > 0.0 {
            (self.frame_timestamps.len() - 1) as f32 / duration.as_secs_f32()
        } else {
            0.0
        }
    }

    /// 计算比特率（bps）
    fn calculate_bitrate(&self) -> u64 {
        if self.frame_timestamps.len() < 2 {
            return 0;
        }
        
        let total_bytes: usize = self.frame_sizes.iter().sum();
        let first = self.frame_timestamps.front().unwrap();
        let last = self.frame_timestamps.back().unwrap();
        let duration = last.duration_since(*first);
        
        if duration.as_secs_f32() > 0.0 {
            ((total_bytes as f64 * 8.0) / duration.as_secs_f64()) as u64
        } else {
            0
        }
    }

    /// 计算质量分数（0-100）
    fn calculate_quality_score(&self, fps: f32, bitrate: u64) -> f32 {
        let mut score = 0.0;
        
        // FPS 评分（满分 40）
        let fps_score = if fps >= 30.0 {
            40.0
        } else if fps >= 24.0 {
            30.0 + (fps - 24.0) / 6.0 * 10.0
        } else if fps >= 15.0 {
            20.0 + (fps - 15.0) / 9.0 * 10.0
        } else {
            fps / 15.0 * 20.0
        };
        score += fps_score;
        
        // 比特率评分（满分 40）
        let bitrate_mbps = bitrate as f32 / 1_000_000.0;
        let bitrate_score = if bitrate_mbps >= 5.0 {
            40.0
        } else if bitrate_mbps >= 2.0 {
            30.0 + (bitrate_mbps - 2.0) / 3.0 * 10.0
        } else if bitrate_mbps >= 1.0 {
            20.0 + (bitrate_mbps - 1.0) * 10.0
        } else {
            bitrate_mbps * 20.0
        };
        score += bitrate_score;
        
        // 丢帧率评分（满分 20）
        let drop_rate = if self.total_frames > 0 {
            (self.dropped_frames as f32 / self.total_frames as f32) * 100.0
        } else {
            0.0
        };
        let drop_score = if drop_rate < 1.0 {
            20.0
        } else if drop_rate < 5.0 {
            15.0 - (drop_rate - 1.0) / 4.0 * 5.0
        } else if drop_rate < 10.0 {
            10.0 - (drop_rate - 5.0) / 5.0 * 5.0
        } else {
            0.0
        };
        score += drop_score;
        
        score.min(100.0)
    }

    /// 重置统计
    pub fn reset(&mut self) {
        self.frame_timestamps.clear();
        self.frame_sizes.clear();
        self.total_bytes = 0;
        self.total_frames = 0;
        self.dropped_frames = 0;
        self.last_frame_time = None;
    }
}

impl Default for QualityMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// 质量指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// 比特率（bps）
    pub bitrate: u64,
    /// 帧率（fps）
    pub fps: f32,
    /// 分辨率（宽, 高）
    pub resolution: (u32, u32),
    /// 质量分数（0-100）
    pub quality_score: f32,
    /// 总帧数
    pub total_frames: u64,
    /// 丢帧数
    pub dropped_frames: u64,
    /// 丢帧率（%）
    pub drop_rate: f32,
}

impl QualityMetrics {
    /// 获取质量等级
    pub fn quality_level(&self) -> QualityLevel {
        match self.quality_score as u32 {
            90..=100 => QualityLevel::Excellent,
            75..=89 => QualityLevel::Good,
            60..=74 => QualityLevel::Fair,
            40..=59 => QualityLevel::Poor,
            _ => QualityLevel::Bad,
        }
    }

    /// 获取比特率（Mbps）
    pub fn bitrate_mbps(&self) -> f32 {
        self.bitrate as f32 / 1_000_000.0
    }
}

/// 质量等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityLevel {
    Excellent,  // 90-100
    Good,       // 75-89
    Fair,       // 60-74
    Poor,       // 40-59
    Bad,        // 0-39
}
