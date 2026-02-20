use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// 自适应码率控制器
pub struct AbrController {
    /// 当前码率（kbps）
    current_bitrate: u32,
    
    /// 可用码率列表
    available_bitrates: Vec<u32>,
    
    /// 带宽估算器
    bandwidth_estimator: BandwidthEstimator,
    
    /// 缓冲区监控器
    buffer_monitor: BufferMonitor,
    
    /// ABR 策略
    strategy: AbrStrategy,
}

/// ABR 策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbrStrategy {
    /// 保守策略（优先稳定）
    Conservative,
    
    /// 平衡策略（默认）
    Balanced,
    
    /// 激进策略（优先质量）
    Aggressive,
}

/// 码率切换决策
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitrateDecision {
    /// 保持当前码率
    Keep,
    
    /// 升级到更高码率
    Upgrade(u32),
    
    /// 降级到更低码率
    Downgrade(u32),
}

impl AbrController {
    pub fn new(available_bitrates: Vec<u32>, strategy: AbrStrategy) -> Self {
        let current_bitrate = *available_bitrates.first().unwrap_or(&500);
        
        Self {
            current_bitrate,
            available_bitrates,
            bandwidth_estimator: BandwidthEstimator::new(),
            buffer_monitor: BufferMonitor::new(Duration::from_secs(10)),
            strategy,
        }
    }

    /// 更新带宽测量
    pub fn update_bandwidth(&mut self, bytes: u64, duration: Duration) {
        self.bandwidth_estimator.add_sample(bytes, duration);
    }

    /// 更新缓冲区状态
    pub fn update_buffer(&mut self, buffer_level: Duration) {
        self.buffer_monitor.update(buffer_level);
    }

    /// 获取码率切换决策
    pub fn get_decision(&self) -> BitrateDecision {
        let estimated_bandwidth = self.bandwidth_estimator.estimate();
        let buffer_level = self.buffer_monitor.current_level();
        let buffer_target = self.buffer_monitor.target_level();

        // 根据策略调整阈值
        let (upgrade_threshold, downgrade_threshold) = match self.strategy {
            AbrStrategy::Conservative => (1.5, 0.8),
            AbrStrategy::Balanced => (1.3, 0.9),
            AbrStrategy::Aggressive => (1.2, 0.95),
        };

        // 缓冲区过低，降级
        if buffer_level < buffer_target / 2 {
            return self.find_lower_bitrate();
        }

        // 带宽充足且缓冲区健康，考虑升级
        if estimated_bandwidth > self.current_bitrate as f64 * upgrade_threshold
            && buffer_level > buffer_target
        {
            return self.find_higher_bitrate(estimated_bandwidth);
        }

        // 带宽不足，降级
        if estimated_bandwidth < self.current_bitrate as f64 * downgrade_threshold {
            return self.find_lower_bitrate();
        }

        BitrateDecision::Keep
    }

    /// 查找更高的码率
    fn find_higher_bitrate(&self, bandwidth: f64) -> BitrateDecision {
        for &bitrate in self.available_bitrates.iter().rev() {
            if bitrate > self.current_bitrate && (bitrate as f64) < bandwidth * 0.9 {
                return BitrateDecision::Upgrade(bitrate);
            }
        }
        BitrateDecision::Keep
    }

    /// 查找更低的码率
    fn find_lower_bitrate(&self) -> BitrateDecision {
        for &bitrate in self.available_bitrates.iter() {
            if bitrate < self.current_bitrate {
                return BitrateDecision::Downgrade(bitrate);
            }
        }
        BitrateDecision::Keep
    }

    /// 应用码率切换决策
    pub fn apply_decision(&mut self, decision: BitrateDecision) -> Option<u32> {
        match decision {
            BitrateDecision::Keep => None,
            BitrateDecision::Upgrade(bitrate) | BitrateDecision::Downgrade(bitrate) => {
                self.current_bitrate = bitrate;
                Some(bitrate)
            }
        }
    }

    /// 获取当前码率
    pub fn current_bitrate(&self) -> u32 {
        self.current_bitrate
    }
}

/// 带宽估算器
struct BandwidthEstimator {
    samples: Vec<BandwidthSample>,
    max_samples: usize,
}

struct BandwidthSample {
    bandwidth_kbps: f64,
    timestamp: Instant,
}

impl BandwidthEstimator {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
            max_samples: 10,
        }
    }

    fn add_sample(&mut self, bytes: u64, duration: Duration) {
        let bandwidth_kbps = (bytes as f64 * 8.0) / (duration.as_secs_f64() * 1000.0);
        
        self.samples.push(BandwidthSample {
            bandwidth_kbps,
            timestamp: Instant::now(),
        });

        // 保持最近的样本
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    fn estimate(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }

        // 使用加权平均（最近的样本权重更高）
        let total_weight: f64 = (1..=self.samples.len()).map(|i| i as f64).sum();
        let weighted_sum: f64 = self
            .samples
            .iter()
            .enumerate()
            .map(|(i, sample)| sample.bandwidth_kbps * (i + 1) as f64)
            .sum();

        weighted_sum / total_weight
    }
}

/// 缓冲区监控器
struct BufferMonitor {
    current_level: Duration,
    target_level: Duration,
}

impl BufferMonitor {
    fn new(target_level: Duration) -> Self {
        Self {
            current_level: Duration::ZERO,
            target_level,
        }
    }

    fn update(&mut self, level: Duration) {
        self.current_level = level;
    }

    fn current_level(&self) -> Duration {
        self.current_level
    }

    fn target_level(&self) -> Duration {
        self.target_level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abr_controller_creation() {
        let bitrates = vec![500, 1000, 2000, 4000];
        let controller = AbrController::new(bitrates, AbrStrategy::Balanced);
        assert_eq!(controller.current_bitrate(), 500);
    }

    #[test]
    fn test_bandwidth_estimator() {
        let mut estimator = BandwidthEstimator::new();
        
        // 添加样本：1MB in 1s = 8000 kbps
        estimator.add_sample(1_000_000, Duration::from_secs(1));
        
        let estimate = estimator.estimate();
        assert!((estimate - 8000.0).abs() < 1.0);
    }

    #[test]
    fn test_upgrade_decision() {
        let bitrates = vec![500, 1000, 2000];
        let mut controller = AbrController::new(bitrates, AbrStrategy::Balanced);
        
        // 模拟高带宽
        controller.update_bandwidth(2_000_000, Duration::from_secs(1)); // 16000 kbps
        controller.update_buffer(Duration::from_secs(15));
        
        let decision = controller.get_decision();
        assert!(matches!(decision, BitrateDecision::Upgrade(_)));
    }

    #[test]
    fn test_downgrade_decision() {
        let bitrates = vec![500, 1000, 2000];
        let mut controller = AbrController::new(bitrates.clone(), AbrStrategy::Balanced);
        controller.current_bitrate = 2000;
        
        // 模拟低带宽
        controller.update_bandwidth(100_000, Duration::from_secs(1)); // 800 kbps
        controller.update_buffer(Duration::from_secs(3));
        
        let decision = controller.get_decision();
        assert!(matches!(decision, BitrateDecision::Downgrade(_)));
    }
}
