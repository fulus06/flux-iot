use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// MQTT 指标收集器
#[derive(Clone)]
pub struct MqttMetrics {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    // 连接指标
    connections_current: AtomicU64,
    connections_total: AtomicU64,
    connections_peak: AtomicU64,
    
    // 消息指标
    messages_published: AtomicU64,
    messages_received: AtomicU64,
    messages_dropped: AtomicU64,
    
    // 字节指标
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    
    // QoS 指标
    qos0_messages: AtomicU64,
    qos1_messages: AtomicU64,
    qos2_messages: AtomicU64,
    
    // Retained 消息指标
    retained_messages: AtomicU64,
    
    // 订阅指标
    subscriptions_current: AtomicU64,
    
    // 启动时间
    start_time: Instant,
}

impl MqttMetrics {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                connections_current: AtomicU64::new(0),
                connections_total: AtomicU64::new(0),
                connections_peak: AtomicU64::new(0),
                messages_published: AtomicU64::new(0),
                messages_received: AtomicU64::new(0),
                messages_dropped: AtomicU64::new(0),
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                qos0_messages: AtomicU64::new(0),
                qos1_messages: AtomicU64::new(0),
                qos2_messages: AtomicU64::new(0),
                retained_messages: AtomicU64::new(0),
                subscriptions_current: AtomicU64::new(0),
                start_time: Instant::now(),
            }),
        }
    }

    // 连接指标
    pub fn record_connection(&self) {
        let current = self.inner.connections_current.fetch_add(1, Ordering::Relaxed) + 1;
        self.inner.connections_total.fetch_add(1, Ordering::Relaxed);
        
        // 更新峰值
        let mut peak = self.inner.connections_peak.load(Ordering::Relaxed);
        while current > peak {
            match self.inner.connections_peak.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }

    pub fn record_disconnection(&self) {
        self.inner.connections_current.fetch_sub(1, Ordering::Relaxed);
    }

    // 消息指标
    pub fn record_message_published(&self, bytes: usize, qos: u8) {
        self.inner.messages_published.fetch_add(1, Ordering::Relaxed);
        self.inner.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
        
        match qos {
            0 => self.inner.qos0_messages.fetch_add(1, Ordering::Relaxed),
            1 => self.inner.qos1_messages.fetch_add(1, Ordering::Relaxed),
            2 => self.inner.qos2_messages.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
    }

    pub fn record_message_received(&self, bytes: usize, qos: u8) {
        self.inner.messages_received.fetch_add(1, Ordering::Relaxed);
        self.inner.bytes_received.fetch_add(bytes as u64, Ordering::Relaxed);
        
        match qos {
            0 => self.inner.qos0_messages.fetch_add(1, Ordering::Relaxed),
            1 => self.inner.qos1_messages.fetch_add(1, Ordering::Relaxed),
            2 => self.inner.qos2_messages.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
    }

    pub fn record_message_dropped(&self) {
        self.inner.messages_dropped.fetch_add(1, Ordering::Relaxed);
    }

    // Retained 消息指标
    pub fn record_retained_message_stored(&self) {
        self.inner.retained_messages.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_retained_message_removed(&self) {
        self.inner.retained_messages.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn set_retained_messages(&self, count: u64) {
        self.inner.retained_messages.store(count, Ordering::Relaxed);
    }

    // 订阅指标
    pub fn record_subscription(&self) {
        self.inner.subscriptions_current.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_unsubscription(&self) {
        self.inner.subscriptions_current.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn set_subscriptions(&self, count: u64) {
        self.inner.subscriptions_current.store(count, Ordering::Relaxed);
    }

    // 获取指标快照
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            connections_current: self.inner.connections_current.load(Ordering::Relaxed),
            connections_total: self.inner.connections_total.load(Ordering::Relaxed),
            connections_peak: self.inner.connections_peak.load(Ordering::Relaxed),
            messages_published: self.inner.messages_published.load(Ordering::Relaxed),
            messages_received: self.inner.messages_received.load(Ordering::Relaxed),
            messages_dropped: self.inner.messages_dropped.load(Ordering::Relaxed),
            bytes_sent: self.inner.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.inner.bytes_received.load(Ordering::Relaxed),
            qos0_messages: self.inner.qos0_messages.load(Ordering::Relaxed),
            qos1_messages: self.inner.qos1_messages.load(Ordering::Relaxed),
            qos2_messages: self.inner.qos2_messages.load(Ordering::Relaxed),
            retained_messages: self.inner.retained_messages.load(Ordering::Relaxed),
            subscriptions_current: self.inner.subscriptions_current.load(Ordering::Relaxed),
            uptime: self.inner.start_time.elapsed(),
        }
    }

    /// 导出 Prometheus 格式的指标
    pub fn export_prometheus(&self) -> String {
        let snapshot = self.snapshot();
        
        format!(
            r#"# HELP mqtt_connections_current Current number of MQTT connections
# TYPE mqtt_connections_current gauge
mqtt_connections_current {}

# HELP mqtt_connections_total Total number of MQTT connections
# TYPE mqtt_connections_total counter
mqtt_connections_total {}

# HELP mqtt_connections_peak Peak number of concurrent MQTT connections
# TYPE mqtt_connections_peak gauge
mqtt_connections_peak {}

# HELP mqtt_messages_published_total Total number of published messages
# TYPE mqtt_messages_published_total counter
mqtt_messages_published_total {}

# HELP mqtt_messages_received_total Total number of received messages
# TYPE mqtt_messages_received_total counter
mqtt_messages_received_total {}

# HELP mqtt_messages_dropped_total Total number of dropped messages
# TYPE mqtt_messages_dropped_total counter
mqtt_messages_dropped_total {}

# HELP mqtt_bytes_sent_total Total bytes sent
# TYPE mqtt_bytes_sent_total counter
mqtt_bytes_sent_total {}

# HELP mqtt_bytes_received_total Total bytes received
# TYPE mqtt_bytes_received_total counter
mqtt_bytes_received_total {}

# HELP mqtt_qos0_messages_total Total QoS 0 messages
# TYPE mqtt_qos0_messages_total counter
mqtt_qos0_messages_total {}

# HELP mqtt_qos1_messages_total Total QoS 1 messages
# TYPE mqtt_qos1_messages_total counter
mqtt_qos1_messages_total {}

# HELP mqtt_qos2_messages_total Total QoS 2 messages
# TYPE mqtt_qos2_messages_total counter
mqtt_qos2_messages_total {}

# HELP mqtt_retained_messages Current number of retained messages
# TYPE mqtt_retained_messages gauge
mqtt_retained_messages {}

# HELP mqtt_subscriptions_current Current number of subscriptions
# TYPE mqtt_subscriptions_current gauge
mqtt_subscriptions_current {}

# HELP mqtt_uptime_seconds Broker uptime in seconds
# TYPE mqtt_uptime_seconds gauge
mqtt_uptime_seconds {}
"#,
            snapshot.connections_current,
            snapshot.connections_total,
            snapshot.connections_peak,
            snapshot.messages_published,
            snapshot.messages_received,
            snapshot.messages_dropped,
            snapshot.bytes_sent,
            snapshot.bytes_received,
            snapshot.qos0_messages,
            snapshot.qos1_messages,
            snapshot.qos2_messages,
            snapshot.retained_messages,
            snapshot.subscriptions_current,
            snapshot.uptime.as_secs(),
        )
    }
}

impl Default for MqttMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 指标快照
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub connections_current: u64,
    pub connections_total: u64,
    pub connections_peak: u64,
    pub messages_published: u64,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub qos0_messages: u64,
    pub qos1_messages: u64,
    pub qos2_messages: u64,
    pub retained_messages: u64,
    pub subscriptions_current: u64,
    pub uptime: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_connection() {
        let metrics = MqttMetrics::new();
        
        metrics.record_connection();
        metrics.record_connection();
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.connections_current, 2);
        assert_eq!(snapshot.connections_total, 2);
        assert_eq!(snapshot.connections_peak, 2);
        
        metrics.record_disconnection();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.connections_current, 1);
    }

    #[test]
    fn test_metrics_messages() {
        let metrics = MqttMetrics::new();
        
        metrics.record_message_published(100, 1);
        metrics.record_message_received(50, 0);
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.messages_published, 1);
        assert_eq!(snapshot.messages_received, 1);
        assert_eq!(snapshot.bytes_sent, 100);
        assert_eq!(snapshot.bytes_received, 50);
        assert_eq!(snapshot.qos0_messages, 1);
        assert_eq!(snapshot.qos1_messages, 1);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = MqttMetrics::new();
        metrics.record_connection();
        
        let prometheus = metrics.export_prometheus();
        assert!(prometheus.contains("mqtt_connections_current 1"));
    }
}
