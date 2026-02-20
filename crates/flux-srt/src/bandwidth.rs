use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// 带宽估计器
pub struct BandwidthEstimator {
    // 发送速率样本（字节/秒）
    send_rate_samples: VecDeque<RateSample>,
    // 接收速率样本
    recv_rate_samples: VecDeque<RateSample>,
    // 样本窗口大小
    window_size: usize,
    // 估计的带宽（字节/秒）
    estimated_bandwidth: Option<u64>,
    // 最后更新时间
    last_update: Instant,
}

#[derive(Debug, Clone)]
struct RateSample {
    timestamp: Instant,
    bytes: u64,
}

impl BandwidthEstimator {
    pub fn new(window_size: usize) -> Self {
        Self {
            send_rate_samples: VecDeque::with_capacity(window_size),
            recv_rate_samples: VecDeque::with_capacity(window_size),
            window_size,
            estimated_bandwidth: None,
            last_update: Instant::now(),
        }
    }

    /// 记录发送的字节数
    pub fn record_sent(&mut self, bytes: u64) {
        let now = Instant::now();
        self.send_rate_samples.push_back(RateSample {
            timestamp: now,
            bytes,
        });

        // 保持窗口大小
        while self.send_rate_samples.len() > self.window_size {
            self.send_rate_samples.pop_front();
        }

        self.update_estimate();
    }

    /// 记录接收的字节数
    pub fn record_received(&mut self, bytes: u64) {
        let now = Instant::now();
        self.recv_rate_samples.push_back(RateSample {
            timestamp: now,
            bytes,
        });

        // 保持窗口大小
        while self.recv_rate_samples.len() > self.window_size {
            self.recv_rate_samples.pop_front();
        }

        self.update_estimate();
    }

    /// 更新带宽估计
    fn update_estimate(&mut self) {
        let now = Instant::now();

        // 每秒更新一次
        if now.duration_since(self.last_update) < Duration::from_secs(1) {
            return;
        }

        self.last_update = now;

        // 计算发送速率
        let send_rate = self.calculate_rate(&self.send_rate_samples);
        let recv_rate = self.calculate_rate(&self.recv_rate_samples);

        // 使用较小的速率作为估计带宽（瓶颈）
        self.estimated_bandwidth = match (send_rate, recv_rate) {
            (Some(s), Some(r)) => Some(s.min(r)),
            (Some(s), None) => Some(s),
            (None, Some(r)) => Some(r),
            (None, None) => None,
        };
    }

    /// 计算速率（字节/秒）
    fn calculate_rate(&self, samples: &VecDeque<RateSample>) -> Option<u64> {
        if samples.len() < 2 {
            return None;
        }

        let first = samples.front()?;
        let last = samples.back()?;

        let duration = last.timestamp.duration_since(first.timestamp);
        if duration.as_secs_f64() < 0.1 {
            return None;
        }

        let total_bytes: u64 = samples.iter().map(|s| s.bytes).sum();
        let rate = (total_bytes as f64 / duration.as_secs_f64()) as u64;

        Some(rate)
    }

    /// 获取估计的带宽（字节/秒）
    pub fn estimated_bandwidth(&self) -> Option<u64> {
        self.estimated_bandwidth
    }

    /// 获取估计的带宽（Mbps）
    pub fn estimated_bandwidth_mbps(&self) -> Option<f64> {
        self.estimated_bandwidth
            .map(|bw| (bw as f64 * 8.0) / 1_000_000.0)
    }

    /// 获取发送速率（字节/秒）
    pub fn send_rate(&self) -> Option<u64> {
        self.calculate_rate(&self.send_rate_samples)
    }

    /// 获取接收速率（字节/秒）
    pub fn recv_rate(&self) -> Option<u64> {
        self.calculate_rate(&self.recv_rate_samples)
    }

    /// 清空样本
    pub fn clear(&mut self) {
        self.send_rate_samples.clear();
        self.recv_rate_samples.clear();
        self.estimated_bandwidth = None;
    }
}

/// 流量整形器
pub struct TrafficShaper {
    // 目标速率（字节/秒）
    target_rate: u64,
    // 令牌桶
    tokens: f64,
    // 桶容量
    bucket_capacity: f64,
    // 最后更新时间
    last_update: Instant,
}

impl TrafficShaper {
    pub fn new(target_rate_mbps: f64) -> Self {
        let target_rate = ((target_rate_mbps * 1_000_000.0) / 8.0) as u64;
        let bucket_capacity = target_rate as f64 * 2.0; // 2 秒的容量

        Self {
            target_rate,
            tokens: bucket_capacity,
            bucket_capacity,
            last_update: Instant::now(),
        }
    }

    /// 检查是否可以发送指定字节数
    pub fn can_send(&mut self, bytes: u64) -> bool {
        self.refill_tokens();
        self.tokens >= bytes as f64
    }

    /// 消耗令牌
    pub fn consume(&mut self, bytes: u64) {
        self.refill_tokens();
        self.tokens -= bytes as f64;
        self.tokens = self.tokens.max(0.0);
    }

    /// 补充令牌
    fn refill_tokens(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();

        // 根据目标速率补充令牌
        let new_tokens = elapsed * self.target_rate as f64;
        self.tokens = (self.tokens + new_tokens).min(self.bucket_capacity);

        self.last_update = now;
    }

    /// 更新目标速率
    pub fn set_target_rate(&mut self, target_rate_mbps: f64) {
        self.target_rate = ((target_rate_mbps * 1_000_000.0) / 8.0) as u64;
        self.bucket_capacity = self.target_rate as f64 * 2.0;
    }

    /// 获取目标速率（Mbps）
    pub fn target_rate_mbps(&self) -> f64 {
        (self.target_rate as f64 * 8.0) / 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_bandwidth_estimator() {
        let mut estimator = BandwidthEstimator::new(10);

        // 模拟发送数据（需要足够的时间间隔才能计算速率）
        for _ in 0..5 {
            estimator.record_sent(1000);
            thread::sleep(Duration::from_millis(50));
        }

        // 应该有发送速率
        let rate = estimator.send_rate();
        assert!(rate.is_some(), "Send rate should be calculated");
    }

    #[test]
    fn test_traffic_shaper() {
        let mut shaper = TrafficShaper::new(10.0); // 10 Mbps

        // 应该可以发送小量数据
        assert!(shaper.can_send(1000));

        // 消耗令牌
        shaper.consume(1000);

        // 等待令牌补充
        thread::sleep(Duration::from_millis(10));
        assert!(shaper.can_send(1000));
    }

    #[test]
    fn test_traffic_shaper_rate_limit() {
        let mut shaper = TrafficShaper::new(1.0); // 1 Mbps = 125000 字节/秒

        // 消耗所有令牌
        let max_bytes = (shaper.bucket_capacity as u64).min(1_000_000);
        shaper.consume(max_bytes);

        // 应该无法立即发送大量数据
        assert!(!shaper.can_send(max_bytes));
    }
}
