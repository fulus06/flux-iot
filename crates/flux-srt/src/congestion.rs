use std::time::{Duration, Instant};

/// 拥塞控制状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionState {
    SlowStart,      // 慢启动
    CongestionAvoidance, // 拥塞避免
    FastRecovery,   // 快速恢复
}

/// 拥塞控制器（基于 AIMD 算法）
pub struct CongestionController {
    // 拥塞窗口（CWND）- 可以发送的未确认包数量
    cwnd: f64,
    // 慢启动阈值
    ssthresh: f64,
    // 当前状态
    state: CongestionState,
    // RTT 统计
    rtt_stats: RttStatistics,
    // 最大窗口大小
    max_window: u32,
    // 丢包计数
    loss_count: u32,
    // 上次丢包时间
    last_loss_time: Option<Instant>,
}

impl CongestionController {
    pub fn new(initial_window: u32, max_window: u32) -> Self {
        Self {
            cwnd: initial_window as f64,
            ssthresh: max_window as f64 / 2.0,
            state: CongestionState::SlowStart,
            rtt_stats: RttStatistics::new(),
            max_window,
            loss_count: 0,
            last_loss_time: None,
        }
    }

    /// 获取当前拥塞窗口大小
    pub fn cwnd(&self) -> u32 {
        self.cwnd.min(self.max_window as f64) as u32
    }

    /// 获取当前状态
    pub fn state(&self) -> CongestionState {
        self.state
    }

    /// 处理 ACK（确认）
    pub fn on_ack(&mut self, acked_packets: u32) {
        match self.state {
            CongestionState::SlowStart => {
                // 慢启动：每收到一个 ACK，CWND 增加 1
                self.cwnd += acked_packets as f64;

                // 如果超过 ssthresh，进入拥塞避免
                if self.cwnd >= self.ssthresh {
                    self.state = CongestionState::CongestionAvoidance;
                }
            }
            CongestionState::CongestionAvoidance => {
                // 拥塞避免：每个 RTT，CWND 增加 1
                self.cwnd += acked_packets as f64 / self.cwnd;
            }
            CongestionState::FastRecovery => {
                // 快速恢复：收到新的 ACK，退出快速恢复
                self.cwnd = self.ssthresh;
                self.state = CongestionState::CongestionAvoidance;
            }
        }

        // 限制最大窗口
        if self.cwnd > self.max_window as f64 {
            self.cwnd = self.max_window as f64;
        }
    }

    /// 处理丢包
    pub fn on_loss(&mut self) {
        self.loss_count += 1;
        self.last_loss_time = Some(Instant::now());

        match self.state {
            CongestionState::SlowStart | CongestionState::CongestionAvoidance => {
                // 乘性减：CWND 减半
                self.ssthresh = (self.cwnd / 2.0).max(2.0);
                self.cwnd = self.ssthresh;
                self.state = CongestionState::FastRecovery;
            }
            CongestionState::FastRecovery => {
                // 已经在快速恢复中，进一步减小窗口
                self.cwnd = (self.cwnd * 0.75).max(2.0);
            }
        }
    }

    /// 更新 RTT 统计
    pub fn update_rtt(&mut self, rtt: Duration) {
        self.rtt_stats.update(rtt);
    }

    /// 获取 RTT 统计
    pub fn rtt_stats(&self) -> &RttStatistics {
        &self.rtt_stats
    }

    /// 获取丢包率
    pub fn loss_rate(&self) -> f64 {
        // 简化计算：基于最近的丢包次数
        if self.loss_count == 0 {
            0.0
        } else {
            self.loss_count as f64 / (self.cwnd * 10.0).max(1.0)
        }
    }

    /// 重置丢包计数
    pub fn reset_loss_count(&mut self) {
        self.loss_count = 0;
    }
}

/// RTT 统计
#[derive(Debug, Clone)]
pub struct RttStatistics {
    // 平滑 RTT（SRTT）
    srtt: Option<Duration>,
    // RTT 方差（RTTVAR）
    rttvar: Option<Duration>,
    // 最小 RTT
    min_rtt: Option<Duration>,
    // 最大 RTT
    max_rtt: Option<Duration>,
    // 样本数
    sample_count: u64,
}

impl RttStatistics {
    pub fn new() -> Self {
        Self {
            srtt: None,
            rttvar: None,
            min_rtt: None,
            max_rtt: None,
            sample_count: 0,
        }
    }

    /// 更新 RTT（使用 RFC 6298 算法）
    pub fn update(&mut self, rtt: Duration) {
        self.sample_count += 1;

        // 更新最小/最大 RTT
        self.min_rtt = Some(self.min_rtt.map_or(rtt, |min| min.min(rtt)));
        self.max_rtt = Some(self.max_rtt.map_or(rtt, |max| max.max(rtt)));

        match self.srtt {
            None => {
                // 第一个样本
                self.srtt = Some(rtt);
                self.rttvar = Some(rtt / 2);
            }
            Some(srtt) => {
                // RFC 6298: RTTVAR = (1 - beta) * RTTVAR + beta * |SRTT - R|
                // SRTT = (1 - alpha) * SRTT + alpha * R
                // alpha = 1/8, beta = 1/4
                let diff = if rtt > srtt {
                    rtt - srtt
                } else {
                    srtt - rtt
                };

                let rttvar = self.rttvar.unwrap_or(Duration::from_millis(0));
                self.rttvar = Some(rttvar * 3 / 4 + diff / 4);
                self.srtt = Some(srtt * 7 / 8 + rtt / 8);
            }
        }
    }

    /// 获取平滑 RTT
    pub fn srtt(&self) -> Option<Duration> {
        self.srtt
    }

    /// 获取 RTT 方差
    pub fn rttvar(&self) -> Option<Duration> {
        self.rttvar
    }

    /// 获取 RTO（重传超时）
    pub fn rto(&self) -> Duration {
        match (self.srtt, self.rttvar) {
            (Some(srtt), Some(rttvar)) => {
                // RTO = SRTT + 4 * RTTVAR
                let rto = srtt + rttvar * 4;
                // 限制在 [200ms, 60s] 范围内
                rto.max(Duration::from_millis(200))
                    .min(Duration::from_secs(60))
            }
            _ => Duration::from_secs(1), // 默认 1 秒
        }
    }

    /// 获取最小 RTT
    pub fn min_rtt(&self) -> Option<Duration> {
        self.min_rtt
    }

    /// 获取最大 RTT
    pub fn max_rtt(&self) -> Option<Duration> {
        self.max_rtt
    }

    /// 获取样本数
    pub fn sample_count(&self) -> u64 {
        self.sample_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_congestion_controller_slow_start() {
        let mut cc = CongestionController::new(2, 100);

        assert_eq!(cc.state(), CongestionState::SlowStart);
        assert_eq!(cc.cwnd(), 2);

        // 慢启动：每个 ACK 增加 1
        cc.on_ack(1);
        assert_eq!(cc.cwnd(), 3);

        cc.on_ack(1);
        assert_eq!(cc.cwnd(), 4);
    }

    #[test]
    fn test_congestion_controller_loss() {
        let mut cc = CongestionController::new(10, 100);

        cc.on_loss();
        assert_eq!(cc.state(), CongestionState::FastRecovery);
        assert_eq!(cc.cwnd(), 5); // 减半
    }

    #[test]
    fn test_rtt_statistics() {
        let mut stats = RttStatistics::new();

        stats.update(Duration::from_millis(100));
        assert_eq!(stats.srtt(), Some(Duration::from_millis(100)));

        stats.update(Duration::from_millis(120));
        // SRTT 应该在 100-120 之间
        let srtt = stats.srtt().unwrap();
        assert!(srtt > Duration::from_millis(100));
        assert!(srtt < Duration::from_millis(120));
    }

    #[test]
    fn test_rto_calculation() {
        let mut stats = RttStatistics::new();

        stats.update(Duration::from_millis(100));
        let rto = stats.rto();

        // RTO 应该大于 RTT
        assert!(rto > Duration::from_millis(100));
        // RTO 应该在合理范围内
        assert!(rto < Duration::from_secs(2));
    }
}
