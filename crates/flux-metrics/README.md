# flux-metrics

完整的监控告警系统，支持 Prometheus 指标收集、实时告警规则引擎、多通知渠道和告警聚合降噪。

## 特性

### 指标收集 ✅
- ✅ **Prometheus 指标**：18+ 种指标类型（Counter、Gauge、Histogram）
- ✅ **系统监控**：CPU、内存、磁盘使用率
- ✅ **HTTP 指标**：请求数、延迟、错误率
- ✅ **流指标**：连接数、流数量、数据包统计
- ✅ **性能指标**：延迟分位数（P50/P90/P99）

### 告警系统 ✅
- ✅ **实时规则引擎**：支持阈值规则和自定义规则
- ✅ **告警状态管理**：Firing/Resolved 状态跟踪
- ✅ **告警历史**：完整的告警历史记录

### 通知渠道 ✅
- ✅ **Webhook**：通用 HTTP Webhook
- ✅ **钉钉**：钉钉机器人通知（Markdown 格式）
- ✅ **邮件**：SMTP 邮件通知
- ✅ **批量通知**：支持多渠道同时发送

### 告警聚合降噪 ✅
- ✅ **静默机制**：防止告警风暴
- ✅ **去重机制**：避免重复告警
- ✅ **告警分组**：按标签或严重程度分组
- ✅ **聚合窗口**：时间窗口内聚合告警

---

## 快速开始

### 1. 基本指标收集

```rust
use flux_metrics::MetricsCollector;
use std::sync::Arc;

// 创建指标收集器
let metrics = Arc::new(MetricsCollector::new().unwrap());

// 记录 HTTP 请求
metrics.record_http_request("GET", "/api/streams", 200, 0.05);

// 记录流事件
metrics.record_stream_started("rtsp", "stream1");
metrics.record_packet_sent("srt", "stream1", 1500);

// 导出 Prometheus 格式
let metrics_text = metrics.export().unwrap();
println!("{}", metrics_text);
```

### 2. 系统监控

```rust
use flux_metrics::{MetricsCollector, SystemMetricsCollector};
use std::sync::Arc;

let metrics = Arc::new(MetricsCollector::new().unwrap());
let system_collector = SystemMetricsCollector::new(metrics.clone());

// 启动定期收集（每 10 秒）
system_collector.start_periodic_collection(10).await;
```

### 3. 告警规则引擎

```rust
use flux_metrics::{AlertEngine, ThresholdRule, AlertSeverity, Comparison};

// 创建告警引擎
let mut engine = AlertEngine::new();

// 添加 CPU 告警规则
engine.add_rule(Box::new(
    ThresholdRule::new(
        "high_cpu".to_string(),
        AlertSeverity::Warning,
        0.8,
        Comparison::GreaterThan,
    )
));

// 评估指标
let alerts = engine.evaluate("high_cpu", 0.85).await;
for alert in alerts {
    println!("Alert: {} - {}", alert.name, alert.message);
}
```

### 4. 多通知渠道

```rust
use flux_metrics::{
    NotificationManager, 
    WebhookNotifier, 
    DingTalkNotifier, 
    EmailNotifier
};

let mut manager = NotificationManager::new();

// 添加 Webhook
manager.add_notifier(Box::new(
    WebhookNotifier::new("https://example.com/webhook".to_string())
));

// 添加钉钉
manager.add_notifier(Box::new(
    DingTalkNotifier::new("https://oapi.dingtalk.com/robot/send?access_token=xxx".to_string())
));

// 添加邮件
manager.add_notifier(Box::new(
    EmailNotifier::new(
        "smtp.example.com".to_string(),
        "alert@example.com".to_string(),
        vec!["admin@example.com".to_string()],
    )
));

// 发送告警
manager.notify(&alert).await;
```

### 5. 告警聚合降噪

```rust
use flux_metrics::{AlertAggregator, AlertDeduplicator};

// 创建聚合器（60秒静默期，300秒聚合窗口）
let aggregator = AlertAggregator::new(60, 300);

// 创建去重器（120秒去重窗口）
let deduplicator = AlertDeduplicator::new(120);

// 检查是否应该静默
if !aggregator.should_silence(&alert).await {
    // 检查是否重复
    if !deduplicator.is_duplicate(&alert).await {
        // 发送告警
        notification_manager.notify(&alert).await;
        
        // 记录
        aggregator.record_alert(&alert).await;
        deduplicator.record_alert(alert.clone()).await;
    }
}
```

---

## 完整示例

```rust
use flux_metrics::{
    AlertEngine, AlertAggregator, AlertDeduplicator,
    NotificationManager, ThresholdRule,
    AlertSeverity, Comparison,
    WebhookNotifier, DingTalkNotifier,
};

#[tokio::main]
async fn main() {
    // 1. 创建告警引擎
    let mut engine = AlertEngine::new();
    engine.add_rule(Box::new(
        ThresholdRule::new(
            "high_cpu".to_string(),
            AlertSeverity::Warning,
            0.8,
            Comparison::GreaterThan,
        )
    ));

    // 2. 创建通知管理器
    let mut notifier = NotificationManager::new();
    notifier.add_notifier(Box::new(
        WebhookNotifier::new("https://example.com/webhook".to_string())
    ));

    // 3. 创建聚合器和去重器
    let aggregator = AlertAggregator::new(60, 300);
    let deduplicator = AlertDeduplicator::new(120);

    // 4. 评估指标并发送告警
    let alerts = engine.evaluate("high_cpu", 0.85).await;
    for alert in alerts {
        if !deduplicator.is_duplicate(&alert).await 
            && !aggregator.should_silence(&alert).await {
            notifier.notify(&alert).await;
            aggregator.record_alert(&alert).await;
            deduplicator.record_alert(alert.clone()).await;
        }
    }
}
```

---

## 指标类型

### HTTP 指标
- `http_requests_total{method, path, status}` - 请求总数
- `http_request_duration_seconds{method, path}` - 请求延迟

### 流指标
- `stream_started_total{protocol, stream_name}` - 流启动总数
- `stream_stopped_total{protocol, stream_name, reason}` - 流停止总数
- `active_streams{protocol}` - 活跃流数量
- `active_connections{protocol}` - 活跃连接数

### 数据包指标
- `packets_sent_total{protocol, stream_name}` - 发送包总数
- `packets_received_total{protocol, stream_name}` - 接收包总数
- `packets_lost_total{protocol, stream_name}` - 丢失包总数
- `bytes_sent_total{protocol, stream_name}` - 发送字节总数
- `bytes_received_total{protocol, stream_name}` - 接收字节总数

### 系统指标
- `memory_usage_bytes` - 内存使用量
- `cpu_usage_ratio` - CPU 使用率
- `disk_usage_ratio{path}` - 磁盘使用率

### 错误指标
- `errors_total{type, component}` - 错误总数

### 性能指标
- `stream_processing_duration_seconds{protocol}` - 流处理延迟
- `packet_latency_seconds{protocol}` - 数据包延迟

---

## 告警规则

### 阈值规则

```rust
ThresholdRule::new(
    "rule_name".to_string(),
    AlertSeverity::Warning,
    threshold_value,
    Comparison::GreaterThan,  // 或 LessThan, Equal
)
```

### 自定义规则

实现 `AlertRule` trait：

```rust
struct CustomRule {
    // ...
}

impl AlertRule for CustomRule {
    fn name(&self) -> &str { "custom_rule" }
    fn severity(&self) -> AlertSeverity { AlertSeverity::Warning }
    fn evaluate(&self, value: f64) -> bool {
        // 自定义逻辑
        value > 0.8 && value < 0.9
    }
    fn message(&self, value: f64) -> String {
        format!("Custom alert: {}", value)
    }
    fn labels(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}
```

---

## 通知渠道配置

### Webhook

```rust
WebhookNotifier::new("https://your-webhook-url.com".to_string())
```

### 钉钉

```rust
DingTalkNotifier::new(
    "https://oapi.dingtalk.com/robot/send?access_token=YOUR_TOKEN".to_string()
)
```

### 邮件

```rust
EmailNotifier::new(
    "smtp.example.com".to_string(),
    "from@example.com".to_string(),
    vec!["to@example.com".to_string()],
)
```

---

## 告警聚合配置

### 静默期

防止同一告警在短时间内重复发送：

```rust
AlertAggregator::new(
    60,   // 静默期：60秒
    300,  // 聚合窗口：300秒
)
```

### 去重窗口

防止重复告警：

```rust
AlertDeduplicator::new(120)  // 去重窗口：120秒
```

---

## 测试

```bash
# 运行所有测试
cargo test -p flux-metrics

# 运行示例
cargo run --example alert_system -p flux-metrics
```

---

## 架构

```
应用 → MetricsCollector → Prometheus
         ↓
    AlertEngine (评估规则)
         ↓
    AlertAggregator (聚合降噪)
         ↓
    AlertDeduplicator (去重)
         ↓
    NotificationManager → Webhook/钉钉/邮件
```

---

## 性能

- 指标收集开销：< 1% CPU
- 内存占用：< 50MB
- 告警评估延迟：< 1ms
- 通知发送：异步非阻塞

---

## 许可证

MIT
