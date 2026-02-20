# 监控和告警完善方案

**日期**: 2026-02-20  
**当前完成度**: 0%  
**目标**: 完整的 Prometheus + Grafana 监控告警系统

---

## 📊 需求分析

### 当前问题
1. **指标不完整**：缺少延迟分位数、吞吐量、资源使用率等关键指标
2. **无可视化**：没有 Grafana Dashboard
3. **无告警**：没有自动告警机制
4. **无历史数据**：无法追溯历史性能问题
5. **无 SLO 监控**：缺少服务质量目标监控

### 目标
- ✅ 完整的 Prometheus 指标体系
- ✅ Grafana Dashboard 模板
- ✅ 自动告警规则
- ✅ SLO/SLA 监控
- ✅ 性能分析工具

---

## 🏗️ 架构设计

### 1. 监控架构

```
应用服务 → Prometheus Exporter → Prometheus → Grafana
                                        ↓
                                   Alertmanager → 告警通知
```

### 2. 指标层次

```
业务指标
├── RTSP 流指标
│   ├── 连接数
│   ├── 流数量
│   ├── 带宽使用
│   └── 错误率
├── SRT 流指标
│   ├── 连接数
│   ├── 丢包率
│   ├── 重传率
│   └── 延迟
└── 存储指标
    ├── 磁盘使用率
    ├── 写入速率
    └── 读取速率

系统指标
├── CPU 使用率
├── 内存使用率
├── 网络 I/O
└── 磁盘 I/O

应用指标
├── 请求延迟（P50/P90/P99）
├── 请求吞吐量（QPS）
├── 错误率
└── 并发连接数
```

---

## 📋 详细设计

### 1. Prometheus 指标定义

#### 1.1 计数器（Counter）

```rust
// HTTP 请求总数
http_requests_total{method, path, status}

// 流启动总数
stream_started_total{protocol, stream_name}

// 流停止总数
stream_stopped_total{protocol, stream_name, reason}

// 错误总数
errors_total{type, component}

// 数据包发送总数
packets_sent_total{protocol, stream_name}

// 数据包接收总数
packets_received_total{protocol, stream_name}

// 字节发送总数
bytes_sent_total{protocol, stream_name}

// 字节接收总数
bytes_received_total{protocol, stream_name}
```

#### 1.2 仪表盘（Gauge）

```rust
// 当前活跃连接数
active_connections{protocol}

// 当前活跃流数量
active_streams{protocol}

// 内存使用量（字节）
memory_usage_bytes{type}

// CPU 使用率（0-1）
cpu_usage_ratio

// 磁盘使用率（0-1）
disk_usage_ratio{path}

// 缓冲区大小
buffer_size{type, stream_name}
```

#### 1.3 直方图（Histogram）

```rust
// HTTP 请求延迟
http_request_duration_seconds{method, path}
  buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]

// 流处理延迟
stream_processing_duration_seconds{protocol}
  buckets: [0.001, 0.01, 0.1, 1.0, 10.0]

// 数据包延迟
packet_latency_seconds{protocol}
  buckets: [0.0001, 0.001, 0.01, 0.1, 1.0]
```

#### 1.4 摘要（Summary）

```rust
// RTT 统计
rtt_seconds{protocol, stream_name}
  quantiles: [0.5, 0.9, 0.99]

// 带宽使用统计
bandwidth_mbps{protocol, stream_name}
  quantiles: [0.5, 0.9, 0.99]
```

### 2. Grafana Dashboard 设计

#### Dashboard 1: 系统概览

**面板**：
1. 总体健康状态（单值面板）
2. 活跃连接数趋势（时间序列）
3. 活跃流数量趋势（时间序列）
4. 请求 QPS（时间序列）
5. 错误率（时间序列）
6. CPU/内存使用率（仪表盘）

#### Dashboard 2: RTSP 协议监控

**面板**：
1. RTSP 连接数（时间序列）
2. RTSP 流数量（时间序列）
3. RTSP 请求延迟 P99（时间序列）
4. RTSP 错误率（时间序列）
5. RTSP 带宽使用（时间序列）
6. RTSP 会话时长分布（热力图）

#### Dashboard 3: SRT 协议监控

**面板**：
1. SRT 连接数（时间序列）
2. SRT 丢包率（时间序列）
3. SRT 重传率（时间序列）
4. SRT RTT 分布（时间序列）
5. SRT 带宽使用（时间序列）
6. SRT 拥塞窗口（时间序列）

#### Dashboard 4: 存储监控

**面板**：
1. 磁盘使用率（仪表盘）
2. 写入速率（时间序列）
3. 读取速率（时间序列）
4. I/O 延迟（时间序列）
5. 快照数量（时间序列）

#### Dashboard 5: 性能分析

**面板**：
1. 请求延迟分位数（P50/P90/P99）
2. 慢请求 Top 10
3. 错误 Top 10
4. 资源使用趋势
5. 并发连接趋势

### 3. 告警规则

#### 3.1 系统级告警

```yaml
# CPU 使用率过高
- alert: HighCPUUsage
  expr: cpu_usage_ratio > 0.8
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "CPU usage is above 80%"
    description: "CPU usage is {{ $value }}%"

# 内存使用率过高
- alert: HighMemoryUsage
  expr: memory_usage_bytes / memory_total_bytes > 0.9
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Memory usage is above 90%"

# 磁盘使用率过高
- alert: HighDiskUsage
  expr: disk_usage_ratio > 0.85
  for: 10m
  labels:
    severity: warning
  annotations:
    summary: "Disk usage is above 85%"
```

#### 3.2 应用级告警

```yaml
# 错误率过高
- alert: HighErrorRate
  expr: rate(errors_total[5m]) > 10
  for: 5m
  labels:
    severity: critical
  annotations:
    summary: "Error rate is too high"
    description: "Error rate: {{ $value }} errors/sec"

# 请求延迟过高
- alert: HighLatency
  expr: histogram_quantile(0.99, http_request_duration_seconds) > 1.0
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "P99 latency is above 1s"

# 服务不可用
- alert: ServiceDown
  expr: up == 0
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "Service is down"
```

#### 3.3 业务级告警

```yaml
# SRT 丢包率过高
- alert: HighSRTPacketLoss
  expr: rate(packets_lost_total[5m]) / rate(packets_sent_total[5m]) > 0.05
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "SRT packet loss rate is above 5%"

# 连接数异常
- alert: ConnectionSpike
  expr: rate(active_connections[5m]) > 100
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Connection spike detected"

# 流异常终止
- alert: StreamAbnormalTermination
  expr: rate(stream_stopped_total{reason="error"}[5m]) > 5
  for: 5m
  labels:
    severity: critical
  annotations:
    summary: "Too many streams terminated abnormally"
```

### 4. SLO 定义

#### 4.1 可用性 SLO

```
目标：99.9% 可用性（每月停机时间 < 43.2 分钟）

指标：
- 成功请求率 > 99.9%
- 服务响应时间 < 1s (P99)
```

#### 4.2 性能 SLO

```
RTSP 协议：
- 连接建立时间 < 100ms (P99)
- 流启动时间 < 500ms (P99)
- 数据传输延迟 < 50ms (P99)

SRT 协议：
- 握手时间 < 200ms (P99)
- 端到端延迟 < 200ms (P99)
- 丢包率 < 1%
```

#### 4.3 可靠性 SLO

```
- 数据丢失率 < 0.01%
- 错误率 < 0.1%
- 重启恢复时间 < 30s
```

---

## 🔧 实现方案

### 1. Prometheus Exporter

```rust
// crates/flux-metrics/src/lib.rs

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec,
    Registry, TextEncoder, Encoder,
};

pub struct MetricsCollector {
    // 计数器
    http_requests_total: CounterVec,
    stream_started_total: CounterVec,
    errors_total: CounterVec,
    
    // 仪表盘
    active_connections: GaugeVec,
    active_streams: GaugeVec,
    memory_usage_bytes: Gauge,
    
    // 直方图
    http_request_duration: HistogramVec,
    stream_processing_duration: HistogramVec,
    
    registry: Registry,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let registry = Registry::new();
        
        // 初始化指标...
        
        Self {
            http_requests_total,
            // ...
            registry,
        }
    }
    
    pub fn record_http_request(&self, method: &str, path: &str, status: u16, duration: f64) {
        self.http_requests_total
            .with_label_values(&[method, path, &status.to_string()])
            .inc();
            
        self.http_request_duration
            .with_label_values(&[method, path])
            .observe(duration);
    }
    
    pub fn export(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}
```

### 2. HTTP Metrics Endpoint

```rust
// 在 main.rs 中添加 /metrics 端点

async fn metrics_handler(
    State(metrics): State<Arc<MetricsCollector>>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        [("Content-Type", "text/plain; version=0.0.4")],
        metrics.export(),
    )
}

// 添加到路由
let app = Router::new()
    .route("/metrics", get(metrics_handler))
    // ...
```

### 3. Grafana Dashboard JSON

```json
{
  "dashboard": {
    "title": "FLUX IOT - System Overview",
    "panels": [
      {
        "title": "Active Connections",
        "targets": [
          {
            "expr": "sum(active_connections)"
          }
        ]
      }
    ]
  }
}
```

---

## 📋 实施计划

### 阶段 1：指标收集器（3-4 天）
- [ ] 创建 flux-metrics crate
- [ ] 实现 MetricsCollector
- [ ] 集成到现有服务
- [ ] 添加 /metrics 端点

### 阶段 2：Grafana Dashboard（2-3 天）
- [ ] 创建系统概览 Dashboard
- [ ] 创建 RTSP 监控 Dashboard
- [ ] 创建 SRT 监控 Dashboard
- [ ] 创建存储监控 Dashboard

### 阶段 3：告警规则（2-3 天）
- [ ] 编写 Prometheus 告警规则
- [ ] 配置 Alertmanager
- [ ] 集成通知渠道（邮件/Slack/钉钉）

### 阶段 4：SLO 监控（2-3 天）
- [ ] 定义 SLO 指标
- [ ] 实现 SLO 计算
- [ ] 创建 SLO Dashboard

### 阶段 5：文档和测试（1-2 天）
- [ ] 编写部署文档
- [ ] 编写使用文档
- [ ] 性能测试

**总计**：10-15 天（2-3 周）

---

## 🎯 成功标准

### 功能完整性
- [x] 完整的指标体系
- [x] Grafana Dashboard
- [x] 告警规则
- [x] SLO 监控

### 性能指标
- 指标收集开销 < 1% CPU
- 内存占用 < 50MB
- 指标导出延迟 < 100ms

### 可用性
- 指标收集不影响主业务
- 支持高并发查询
- 历史数据保留 30 天

---

## 📚 依赖库

```toml
[dependencies]
prometheus = "0.13"
lazy_static = "1.4"
sysinfo = "0.30"  # 系统信息收集
```

---

## 🔄 部署架构

```yaml
# docker-compose.yml

services:
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.retention.time=30d'
  
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    volumes:
      - grafana_data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
  
  alertmanager:
    image: prom/alertmanager:latest
    ports:
      - "9093:9093"
    volumes:
      - ./alertmanager.yml:/etc/alertmanager/alertmanager.yml
```

---

## ⚠️ 注意事项

### 1. 性能影响
- 使用 lazy_static 避免重复创建指标
- 批量更新指标减少锁竞争
- 异步导出避免阻塞主线程

### 2. 数据保留
- Prometheus 默认保留 15 天
- 建议配置 30 天保留期
- 长期数据可导出到 InfluxDB

### 3. 告警疲劳
- 合理设置告警阈值
- 使用告警分组
- 配置告警静默期

---

## 🎉 总结

监控和告警系统将提供：
- ✅ 完整的可观测性
- ✅ 实时性能监控
- ✅ 自动告警通知
- ✅ SLO 质量保障
- ✅ 问题快速定位

**预计工期**：2-3 周  
**优先级**：高  
**复杂度**：中等

---

**下一步行动**：
1. 创建 flux-metrics crate
2. 实现 MetricsCollector
3. 集成到现有服务
4. 创建 Grafana Dashboard

**规划完成时间**: 2026-02-20
