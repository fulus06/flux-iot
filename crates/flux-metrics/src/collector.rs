use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, Opts,
    Registry, TextEncoder,
};
use std::sync::Arc;
use tracing::warn;

/// 指标收集器
pub struct MetricsCollector {
    // HTTP 指标
    http_requests_total: CounterVec,
    http_request_duration: HistogramVec,

    // 流指标
    stream_started_total: CounterVec,
    stream_stopped_total: CounterVec,
    active_streams: GaugeVec,
    active_connections: GaugeVec,

    // 数据包指标
    packets_sent_total: CounterVec,
    packets_received_total: CounterVec,
    packets_lost_total: CounterVec,
    bytes_sent_total: CounterVec,
    bytes_received_total: CounterVec,

    // 系统指标
    memory_usage_bytes: Gauge,
    cpu_usage_ratio: Gauge,
    disk_usage_ratio: GaugeVec,

    // 错误指标
    errors_total: CounterVec,

    // 性能指标
    stream_processing_duration: HistogramVec,
    packet_latency: HistogramVec,

    registry: Registry,
}

impl MetricsCollector {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // HTTP 指标
        let http_requests_total = CounterVec::new(
            Opts::new("http_requests_total", "Total number of HTTP requests"),
            &["method", "path", "status"],
        )?;
        registry.register(Box::new(http_requests_total.clone()))?;

        let http_request_duration = HistogramVec::new(
            HistogramOpts::new("http_request_duration_seconds", "HTTP request duration")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["method", "path"],
        )?;
        registry.register(Box::new(http_request_duration.clone()))?;

        // 流指标
        let stream_started_total = CounterVec::new(
            Opts::new("stream_started_total", "Total number of streams started"),
            &["protocol", "stream_name"],
        )?;
        registry.register(Box::new(stream_started_total.clone()))?;

        let stream_stopped_total = CounterVec::new(
            Opts::new("stream_stopped_total", "Total number of streams stopped"),
            &["protocol", "stream_name", "reason"],
        )?;
        registry.register(Box::new(stream_stopped_total.clone()))?;

        let active_streams = GaugeVec::new(
            Opts::new("active_streams", "Number of active streams"),
            &["protocol"],
        )?;
        registry.register(Box::new(active_streams.clone()))?;

        let active_connections = GaugeVec::new(
            Opts::new("active_connections", "Number of active connections"),
            &["protocol"],
        )?;
        registry.register(Box::new(active_connections.clone()))?;

        // 数据包指标
        let packets_sent_total = CounterVec::new(
            Opts::new("packets_sent_total", "Total number of packets sent"),
            &["protocol", "stream_name"],
        )?;
        registry.register(Box::new(packets_sent_total.clone()))?;

        let packets_received_total = CounterVec::new(
            Opts::new("packets_received_total", "Total number of packets received"),
            &["protocol", "stream_name"],
        )?;
        registry.register(Box::new(packets_received_total.clone()))?;

        let packets_lost_total = CounterVec::new(
            Opts::new("packets_lost_total", "Total number of packets lost"),
            &["protocol", "stream_name"],
        )?;
        registry.register(Box::new(packets_lost_total.clone()))?;

        let bytes_sent_total = CounterVec::new(
            Opts::new("bytes_sent_total", "Total number of bytes sent"),
            &["protocol", "stream_name"],
        )?;
        registry.register(Box::new(bytes_sent_total.clone()))?;

        let bytes_received_total = CounterVec::new(
            Opts::new("bytes_received_total", "Total number of bytes received"),
            &["protocol", "stream_name"],
        )?;
        registry.register(Box::new(bytes_received_total.clone()))?;

        // 系统指标
        let memory_usage_bytes = Gauge::new("memory_usage_bytes", "Memory usage in bytes")?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;

        let cpu_usage_ratio = Gauge::new("cpu_usage_ratio", "CPU usage ratio (0-1)")?;
        registry.register(Box::new(cpu_usage_ratio.clone()))?;

        let disk_usage_ratio = GaugeVec::new(
            Opts::new("disk_usage_ratio", "Disk usage ratio (0-1)"),
            &["path"],
        )?;
        registry.register(Box::new(disk_usage_ratio.clone()))?;

        // 错误指标
        let errors_total = CounterVec::new(
            Opts::new("errors_total", "Total number of errors"),
            &["type", "component"],
        )?;
        registry.register(Box::new(errors_total.clone()))?;

        // 性能指标
        let stream_processing_duration = HistogramVec::new(
            HistogramOpts::new(
                "stream_processing_duration_seconds",
                "Stream processing duration",
            )
            .buckets(vec![0.001, 0.01, 0.1, 1.0, 10.0]),
            &["protocol"],
        )?;
        registry.register(Box::new(stream_processing_duration.clone()))?;

        let packet_latency = HistogramVec::new(
            HistogramOpts::new("packet_latency_seconds", "Packet latency")
                .buckets(vec![0.0001, 0.001, 0.01, 0.1, 1.0]),
            &["protocol"],
        )?;
        registry.register(Box::new(packet_latency.clone()))?;

        Ok(Self {
            http_requests_total,
            http_request_duration,
            stream_started_total,
            stream_stopped_total,
            active_streams,
            active_connections,
            packets_sent_total,
            packets_received_total,
            packets_lost_total,
            bytes_sent_total,
            bytes_received_total,
            memory_usage_bytes,
            cpu_usage_ratio,
            disk_usage_ratio,
            errors_total,
            stream_processing_duration,
            packet_latency,
            registry,
        })
    }

    // HTTP 指标记录
    pub fn record_http_request(&self, method: &str, path: &str, status: u16, duration: f64) {
        self.http_requests_total
            .with_label_values(&[method, path, &status.to_string()])
            .inc();

        self.http_request_duration
            .with_label_values(&[method, path])
            .observe(duration);
    }

    // 流指标记录
    pub fn record_stream_started(&self, protocol: &str, stream_name: &str) {
        self.stream_started_total
            .with_label_values(&[protocol, stream_name])
            .inc();

        self.active_streams.with_label_values(&[protocol]).inc();
    }

    pub fn record_stream_stopped(&self, protocol: &str, stream_name: &str, reason: &str) {
        self.stream_stopped_total
            .with_label_values(&[protocol, stream_name, reason])
            .inc();

        self.active_streams.with_label_values(&[protocol]).dec();
    }

    pub fn set_active_connections(&self, protocol: &str, count: i64) {
        self.active_connections
            .with_label_values(&[protocol])
            .set(count as f64);
    }

    // 数据包指标记录
    pub fn record_packet_sent(&self, protocol: &str, stream_name: &str, bytes: u64) {
        self.packets_sent_total
            .with_label_values(&[protocol, stream_name])
            .inc();

        self.bytes_sent_total
            .with_label_values(&[protocol, stream_name])
            .inc_by(bytes as f64);
    }

    pub fn record_packet_received(&self, protocol: &str, stream_name: &str, bytes: u64) {
        self.packets_received_total
            .with_label_values(&[protocol, stream_name])
            .inc();

        self.bytes_received_total
            .with_label_values(&[protocol, stream_name])
            .inc_by(bytes as f64);
    }

    pub fn record_packet_lost(&self, protocol: &str, stream_name: &str, count: u64) {
        self.packets_lost_total
            .with_label_values(&[protocol, stream_name])
            .inc_by(count as f64);
    }

    // 系统指标记录
    pub fn set_memory_usage(&self, bytes: u64) {
        self.memory_usage_bytes.set(bytes as f64);
    }

    pub fn set_cpu_usage(&self, ratio: f64) {
        self.cpu_usage_ratio.set(ratio);
    }

    pub fn set_disk_usage(&self, path: &str, ratio: f64) {
        self.disk_usage_ratio
            .with_label_values(&[path])
            .set(ratio);
    }

    // 错误指标记录
    pub fn record_error(&self, error_type: &str, component: &str) {
        self.errors_total
            .with_label_values(&[error_type, component])
            .inc();
    }

    // 性能指标记录
    pub fn record_stream_processing_duration(&self, protocol: &str, duration: f64) {
        self.stream_processing_duration
            .with_label_values(&[protocol])
            .observe(duration);
    }

    pub fn record_packet_latency(&self, protocol: &str, latency: f64) {
        self.packet_latency
            .with_label_values(&[protocol])
            .observe(latency);
    }

    // 导出指标
    pub fn export(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer).unwrap())
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new().expect("Failed to create MetricsCollector")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new().unwrap();
        assert!(!collector.export().unwrap().is_empty());
    }

    #[test]
    fn test_http_metrics() {
        let collector = MetricsCollector::new().unwrap();

        collector.record_http_request("GET", "/api/streams", 200, 0.05);
        collector.record_http_request("POST", "/api/streams", 201, 0.1);

        let metrics = collector.export().unwrap();
        assert!(metrics.contains("http_requests_total"));
        assert!(metrics.contains("http_request_duration_seconds"));
    }

    #[test]
    fn test_stream_metrics() {
        let collector = MetricsCollector::new().unwrap();

        collector.record_stream_started("rtsp", "stream1");
        collector.record_stream_stopped("rtsp", "stream1", "normal");

        let metrics = collector.export().unwrap();
        assert!(metrics.contains("stream_started_total"));
        assert!(metrics.contains("stream_stopped_total"));
    }

    #[test]
    fn test_packet_metrics() {
        let collector = MetricsCollector::new().unwrap();

        collector.record_packet_sent("srt", "stream1", 1500);
        collector.record_packet_received("srt", "stream1", 1500);
        collector.record_packet_lost("srt", "stream1", 1);

        let metrics = collector.export().unwrap();
        assert!(metrics.contains("packets_sent_total"));
        assert!(metrics.contains("bytes_sent_total"));
    }
}
