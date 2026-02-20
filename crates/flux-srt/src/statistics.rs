use std::time::{Duration, Instant};

/// SRT 连接统计信息
#[derive(Debug, Clone)]
pub struct SrtStatistics {
    // 连接信息
    pub connection_time: Instant,
    pub local_socket_id: u32,
    pub remote_socket_id: Option<u32>,

    // 发送统计
    pub packets_sent: u64,
    pub packets_sent_unique: u64,
    pub packets_retransmitted: u64,
    pub bytes_sent: u64,

    // 接收统计
    pub packets_received: u64,
    pub packets_received_unique: u64,
    pub packets_received_duplicate: u64,
    pub bytes_received: u64,

    // 丢包统计
    pub packets_lost: u64,
    pub loss_rate: f64,

    // RTT 统计
    pub rtt: Option<Duration>,
    pub rtt_variance: Option<Duration>,

    // 带宽统计
    pub send_rate: Option<u64>,      // 字节/秒
    pub recv_rate: Option<u64>,      // 字节/秒
    pub estimated_bandwidth: Option<u64>, // 字节/秒

    // 缓冲区统计
    pub send_buffer_size: usize,
    pub recv_buffer_size: usize,

    // 拥塞控制
    pub cwnd: u32,
    pub ssthresh: u32,
}

impl SrtStatistics {
    pub fn new(local_socket_id: u32) -> Self {
        Self {
            connection_time: Instant::now(),
            local_socket_id,
            remote_socket_id: None,
            packets_sent: 0,
            packets_sent_unique: 0,
            packets_retransmitted: 0,
            bytes_sent: 0,
            packets_received: 0,
            packets_received_unique: 0,
            packets_received_duplicate: 0,
            bytes_received: 0,
            packets_lost: 0,
            loss_rate: 0.0,
            rtt: None,
            rtt_variance: None,
            send_rate: None,
            recv_rate: None,
            estimated_bandwidth: None,
            send_buffer_size: 0,
            recv_buffer_size: 0,
            cwnd: 0,
            ssthresh: 0,
        }
    }

    /// 记录发送的包
    pub fn record_sent(&mut self, bytes: u64, is_retransmit: bool) {
        self.packets_sent += 1;
        self.bytes_sent += bytes;

        if is_retransmit {
            self.packets_retransmitted += 1;
        } else {
            self.packets_sent_unique += 1;
        }
    }

    /// 记录接收的包
    pub fn record_received(&mut self, bytes: u64, is_duplicate: bool) {
        self.packets_received += 1;
        self.bytes_received += bytes;

        if is_duplicate {
            self.packets_received_duplicate += 1;
        } else {
            self.packets_received_unique += 1;
        }
    }

    /// 记录丢包
    pub fn record_loss(&mut self, count: u64) {
        self.packets_lost += count;
        self.update_loss_rate();
    }

    /// 更新丢包率
    fn update_loss_rate(&mut self) {
        let total = self.packets_sent_unique + self.packets_lost;
        if total > 0 {
            self.loss_rate = self.packets_lost as f64 / total as f64;
        }
    }

    /// 更新 RTT
    pub fn update_rtt(&mut self, rtt: Duration, variance: Duration) {
        self.rtt = Some(rtt);
        self.rtt_variance = Some(variance);
    }

    /// 更新带宽
    pub fn update_bandwidth(&mut self, send_rate: Option<u64>, recv_rate: Option<u64>, estimated: Option<u64>) {
        self.send_rate = send_rate;
        self.recv_rate = recv_rate;
        self.estimated_bandwidth = estimated;
    }

    /// 更新缓冲区大小
    pub fn update_buffer_sizes(&mut self, send_buf: usize, recv_buf: usize) {
        self.send_buffer_size = send_buf;
        self.recv_buffer_size = recv_buf;
    }

    /// 更新拥塞控制参数
    pub fn update_congestion(&mut self, cwnd: u32, ssthresh: u32) {
        self.cwnd = cwnd;
        self.ssthresh = ssthresh;
    }

    /// 获取连接时长
    pub fn connection_duration(&self) -> Duration {
        Instant::now().duration_since(self.connection_time)
    }

    /// 获取平均发送速率（Mbps）
    pub fn avg_send_rate_mbps(&self) -> f64 {
        let duration = self.connection_duration().as_secs_f64();
        if duration > 0.0 {
            (self.bytes_sent as f64 * 8.0) / (duration * 1_000_000.0)
        } else {
            0.0
        }
    }

    /// 获取平均接收速率（Mbps）
    pub fn avg_recv_rate_mbps(&self) -> f64 {
        let duration = self.connection_duration().as_secs_f64();
        if duration > 0.0 {
            (self.bytes_received as f64 * 8.0) / (duration * 1_000_000.0)
        } else {
            0.0
        }
    }

    /// 获取重传率
    pub fn retransmit_rate(&self) -> f64 {
        if self.packets_sent > 0 {
            self.packets_retransmitted as f64 / self.packets_sent as f64
        } else {
            0.0
        }
    }

    /// 导出为 JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "connection": {
                "duration_secs": self.connection_duration().as_secs(),
                "local_socket_id": format!("{:08x}", self.local_socket_id),
                "remote_socket_id": self.remote_socket_id.map(|id| format!("{:08x}", id)),
            },
            "send": {
                "packets_total": self.packets_sent,
                "packets_unique": self.packets_sent_unique,
                "packets_retransmitted": self.packets_retransmitted,
                "bytes": self.bytes_sent,
                "rate_mbps": self.avg_send_rate_mbps(),
                "retransmit_rate": self.retransmit_rate(),
            },
            "receive": {
                "packets_total": self.packets_received,
                "packets_unique": self.packets_received_unique,
                "packets_duplicate": self.packets_received_duplicate,
                "bytes": self.bytes_received,
                "rate_mbps": self.avg_recv_rate_mbps(),
            },
            "loss": {
                "packets_lost": self.packets_lost,
                "loss_rate": self.loss_rate,
            },
            "rtt": {
                "rtt_ms": self.rtt.map(|r| r.as_millis()),
                "rtt_variance_ms": self.rtt_variance.map(|r| r.as_millis()),
            },
            "bandwidth": {
                "send_rate_mbps": self.send_rate.map(|r| (r as f64 * 8.0) / 1_000_000.0),
                "recv_rate_mbps": self.recv_rate.map(|r| (r as f64 * 8.0) / 1_000_000.0),
                "estimated_mbps": self.estimated_bandwidth.map(|r| (r as f64 * 8.0) / 1_000_000.0),
            },
            "buffer": {
                "send_buffer_size": self.send_buffer_size,
                "recv_buffer_size": self.recv_buffer_size,
            },
            "congestion": {
                "cwnd": self.cwnd,
                "ssthresh": self.ssthresh,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_creation() {
        let stats = SrtStatistics::new(0x12345678);
        assert_eq!(stats.local_socket_id, 0x12345678);
        assert_eq!(stats.packets_sent, 0);
        assert_eq!(stats.packets_received, 0);
    }

    #[test]
    fn test_record_sent() {
        let mut stats = SrtStatistics::new(1);

        stats.record_sent(1000, false);
        assert_eq!(stats.packets_sent, 1);
        assert_eq!(stats.packets_sent_unique, 1);
        assert_eq!(stats.bytes_sent, 1000);

        stats.record_sent(1000, true);
        assert_eq!(stats.packets_sent, 2);
        assert_eq!(stats.packets_retransmitted, 1);
    }

    #[test]
    fn test_loss_rate() {
        let mut stats = SrtStatistics::new(1);

        stats.record_sent(1000, false);
        stats.record_sent(1000, false);
        stats.record_loss(1);

        assert_eq!(stats.packets_lost, 1);
        assert_eq!(stats.loss_rate, 1.0 / 3.0);
    }

    #[test]
    fn test_retransmit_rate() {
        let mut stats = SrtStatistics::new(1);

        stats.record_sent(1000, false);
        stats.record_sent(1000, false);
        stats.record_sent(1000, true);

        assert_eq!(stats.retransmit_rate(), 1.0 / 3.0);
    }

    #[test]
    fn test_json_export() {
        let mut stats = SrtStatistics::new(0x12345678);
        stats.record_sent(1000, false);
        stats.record_received(1000, false);

        let json = stats.to_json();
        assert!(json["connection"]["local_socket_id"].is_string());
        assert_eq!(json["send"]["packets_total"], 1);
        assert_eq!(json["receive"]["packets_total"], 1);
    }
}
