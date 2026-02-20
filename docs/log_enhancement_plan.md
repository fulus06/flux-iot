# 日志增强实现方案

**日期**: 2026-02-20  
**当前完成度**: 0%  
**目标**: 完整的结构化日志、采样、聚合和分布式追踪系统

---

## 📊 需求分析

### 当前问题
1. **日志格式不统一**：纯文本日志，难以解析和查询
2. **日志量过大**：高频日志导致存储和性能问题
3. **缺少追踪能力**：无法跟踪请求的完整链路
4. **缺少上下文**：日志之间缺少关联，难以定位问题
5. **查询困难**：无法高效查询和分析日志

### 目标
- ✅ 结构化日志（JSON Lines 格式）
- ✅ 日志采样和降噪
- ✅ 分布式追踪（OpenTelemetry）
- ✅ trace_id/span_id 关联
- ✅ 日志聚合和查询

---

## 🏗️ 架构设计

### 1. 日志架构

```
应用 → StructuredLogger → JSON Lines → 文件/Stdout
         ↓
    LogSampler (采样)
         ↓
    OpenTelemetry (追踪)
         ↓
    Jaeger/Zipkin (可视化)
```

### 2. 日志层次

```
结构化日志
├── 基础字段
│   ├── timestamp
│   ├── level
│   ├── message
│   └── target
├── 追踪字段
│   ├── trace_id
│   ├── span_id
│   └── parent_span_id
├── 上下文字段
│   ├── service_name
│   ├── host
│   └── environment
└── 自定义字段
    ├── user_id
    ├── request_id
    └── 业务字段
```

---

## 📋 详细设计

### 1. 结构化日志

#### 1.1 JSON Lines 格式

```json
{
  "timestamp": "2026-02-20T19:40:00.123Z",
  "level": "INFO",
  "message": "Request processed successfully",
  "target": "flux_iot::api",
  "trace_id": "a1b2c3d4e5f6",
  "span_id": "1234567890",
  "service": "flux-iot",
  "host": "server1",
  "request_id": "req-123",
  "duration_ms": 45,
  "status": 200
}
```

#### 1.2 日志级别

```rust
pub enum LogLevel {
    Trace,   // 最详细的调试信息
    Debug,   // 调试信息
    Info,    // 一般信息
    Warn,    // 警告信息
    Error,   // 错误信息
}
```

#### 1.3 结构化字段

```rust
pub struct LogEntry {
    // 基础字段
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub target: String,
    
    // 追踪字段
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
    
    // 上下文字段
    pub service_name: String,
    pub host: String,
    pub environment: String,
    
    // 自定义字段
    pub fields: HashMap<String, serde_json::Value>,
}
```

### 2. 日志采样

#### 2.1 采样策略

```rust
pub enum SamplingStrategy {
    // 始终记录
    Always,
    
    // 从不记录
    Never,
    
    // 按比例采样（0.0-1.0）
    Ratio(f64),
    
    // 按级别采样
    ByLevel {
        trace: f64,
        debug: f64,
        info: f64,
        warn: f64,
        error: f64,
    },
    
    // 速率限制（每秒最多 N 条）
    RateLimit(u32),
    
    // 自适应采样
    Adaptive {
        base_rate: f64,
        max_rate: f64,
        error_boost: f64,
    },
}
```

#### 2.2 采样器实现

```rust
pub struct LogSampler {
    strategy: SamplingStrategy,
    counter: AtomicU64,
    last_reset: Arc<RwLock<Instant>>,
}

impl LogSampler {
    pub fn should_sample(&self, level: LogLevel) -> bool {
        match &self.strategy {
            SamplingStrategy::Always => true,
            SamplingStrategy::Never => false,
            SamplingStrategy::Ratio(ratio) => {
                rand::random::<f64>() < *ratio
            }
            SamplingStrategy::ByLevel { .. } => {
                // 根据级别决定
            }
            SamplingStrategy::RateLimit(max_per_sec) => {
                // 速率限制
            }
            SamplingStrategy::Adaptive { .. } => {
                // 自适应采样
            }
        }
    }
}
```

### 3. OpenTelemetry 集成

#### 3.1 Tracer 配置

```rust
use opentelemetry::{
    global,
    sdk::{
        trace::{self, Sampler},
        Resource,
    },
    KeyValue,
};
use opentelemetry_jaeger::JaegerPipeline;

pub fn init_tracer(service_name: &str) -> Result<()> {
    global::set_text_map_propagator(
        opentelemetry_jaeger::Propagator::new()
    );

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name(service_name)
        .with_agent_endpoint("localhost:6831")
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", service_name),
                    KeyValue::new("service.version", "0.1.0"),
                ]))
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    Ok(())
}
```

#### 3.2 Span 创建

```rust
use opentelemetry::trace::{Tracer, Span};

pub fn create_span(name: &str) -> impl Span {
    let tracer = global::tracer("flux-iot");
    let span = tracer.start(name);
    
    // 添加属性
    span.set_attribute(KeyValue::new("component", "api"));
    span.set_attribute(KeyValue::new("http.method", "GET"));
    
    span
}
```

#### 3.3 上下文传播

```rust
use opentelemetry::Context;

pub async fn handle_request(ctx: Context) -> Result<Response> {
    let _guard = ctx.attach();
    
    // 创建子 Span
    let span = create_span("handle_request");
    
    // 业务逻辑
    process_request().await?;
    
    span.end();
    Ok(response)
}
```

### 4. trace_id/span_id 关联

#### 4.1 日志关联

```rust
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub fn log_with_trace(message: &str) {
    let span = tracing::Span::current();
    let context = span.context();
    
    let trace_id = context.span().span_context().trace_id().to_string();
    let span_id = context.span().span_context().span_id().to_string();
    
    tracing::info!(
        trace_id = %trace_id,
        span_id = %span_id,
        "{}",
        message
    );
}
```

#### 4.2 HTTP Header 传播

```rust
use opentelemetry::propagation::TextMapPropagator;

pub fn extract_trace_context(headers: &HeaderMap) -> Context {
    let propagator = global::get_text_map_propagator(|prop| prop.clone());
    let context = propagator.extract(&HeaderExtractor(headers));
    context
}

pub fn inject_trace_context(headers: &mut HeaderMap, context: &Context) {
    let propagator = global::get_text_map_propagator(|prop| prop.clone());
    propagator.inject_context(context, &mut HeaderInjector(headers));
}
```

### 5. 日志聚合

#### 5.1 日志收集器

```rust
pub struct LogAggregator {
    buffer: Arc<RwLock<Vec<LogEntry>>>,
    max_buffer_size: usize,
    flush_interval: Duration,
}

impl LogAggregator {
    pub async fn add_log(&self, entry: LogEntry) {
        let mut buffer = self.buffer.write().await;
        buffer.push(entry);
        
        if buffer.len() >= self.max_buffer_size {
            self.flush().await;
        }
    }
    
    pub async fn flush(&self) {
        let mut buffer = self.buffer.write().await;
        let logs = std::mem::take(&mut *buffer);
        
        // 写入文件或发送到日志系统
        self.write_logs(logs).await;
    }
}
```

---

## 🔧 实现方案

### 1. 创建 flux-logging crate

```toml
[package]
name = "flux-logging"
version = "0.1.0"
edition = "2021"

[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-opentelemetry = "0.21"
opentelemetry = { version = "0.21", features = ["trace"] }
opentelemetry-jaeger = { version = "0.20", features = ["rt-tokio"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1.35", features = ["full"] }
rand = "0.8"
```

### 2. 核心模块

```
flux-logging/
├── src/
│   ├── lib.rs              # 模块导出
│   ├── structured.rs       # 结构化日志
│   ├── sampler.rs          # 日志采样
│   ├── tracer.rs           # OpenTelemetry 集成
│   ├── aggregator.rs       # 日志聚合
│   └── formatter.rs        # 日志格式化
```

### 3. 使用示例

```rust
use flux_logging::{
    init_logging, LogSampler, SamplingStrategy,
    create_span, log_with_trace,
};

#[tokio::main]
async fn main() {
    // 初始化日志系统
    init_logging("flux-iot", "production").await.unwrap();
    
    // 创建采样器
    let sampler = LogSampler::new(SamplingStrategy::Ratio(0.1));
    
    // 创建 Span
    let span = create_span("main");
    let _guard = span.enter();
    
    // 记录日志（自动关联 trace_id）
    log_with_trace("Application started");
    
    // 业务逻辑
    handle_request().await;
}

async fn handle_request() {
    let span = create_span("handle_request");
    let _guard = span.enter();
    
    tracing::info!(
        request_id = "req-123",
        user_id = "user-456",
        "Processing request"
    );
    
    // 业务逻辑
}
```

---

## 📋 实施计划

### 阶段 1：结构化日志（2-3 天）
- [ ] 创建 flux-logging crate
- [ ] 实现 LogEntry 结构
- [ ] 实现 JSON Lines 格式化
- [ ] 集成 tracing-subscriber

### 阶段 2：日志采样（2-3 天）
- [ ] 实现 LogSampler
- [ ] 实现多种采样策略
- [ ] 实现速率限制
- [ ] 实现自适应采样

### 阶段 3：OpenTelemetry 集成（3-4 天）
- [ ] 集成 opentelemetry-jaeger
- [ ] 实现 Tracer 初始化
- [ ] 实现 Span 创建和管理
- [ ] 实现上下文传播

### 阶段 4：trace_id 关联（2-3 天）
- [ ] 实现日志与 trace 关联
- [ ] 实现 HTTP Header 传播
- [ ] 实现跨服务追踪

### 阶段 5：日志聚合（2-3 天）
- [ ] 实现 LogAggregator
- [ ] 实现批量写入
- [ ] 实现定期刷新

### 阶段 6：测试和文档（1-2 天）
- [ ] 单元测试
- [ ] 集成测试
- [ ] 使用文档
- [ ] 示例代码

**总计**：12-18 天（2.5-3.5 周）

---

## 🎯 成功标准

### 功能完整性
- [x] 结构化日志（JSON Lines）
- [x] 日志采样
- [x] OpenTelemetry 集成
- [x] trace_id/span_id 关联
- [x] 日志聚合

### 性能指标
- 日志开销 < 5% CPU
- 采样后日志量减少 80%+
- 追踪开销 < 2% CPU

### 可用性
- 易于集成
- 配置灵活
- 文档完善

---

## 📚 依赖库

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-opentelemetry = "0.21"
opentelemetry = { version = "0.21", features = ["trace"] }
opentelemetry-jaeger = { version = "0.20", features = ["rt-tokio"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1.35", features = ["full"] }
rand = "0.8"
```

---

## 🔄 部署架构

```yaml
# docker-compose.yml

services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "6831:6831/udp"  # Jaeger agent
      - "16686:16686"     # Jaeger UI
    environment:
      - COLLECTOR_ZIPKIN_HOST_PORT=:9411
  
  flux-iot:
    build: .
    environment:
      - RUST_LOG=info
      - OTEL_EXPORTER_JAEGER_AGENT_HOST=jaeger
      - OTEL_EXPORTER_JAEGER_AGENT_PORT=6831
    depends_on:
      - jaeger
```

---

## ⚠️ 注意事项

### 1. 性能影响
- 结构化日志比纯文本慢 10-20%
- 使用采样减少日志量
- 异步写入避免阻塞

### 2. 存储成本
- JSON 格式比纯文本大 30-50%
- 使用压缩减少存储
- 定期清理旧日志

### 3. 追踪开销
- OpenTelemetry 有一定开销
- 使用采样减少追踪数据
- 生产环境建议采样率 1-10%

---

## 🎉 总结

日志增强系统将提供：
- ✅ 结构化日志（易于查询）
- ✅ 日志采样（减少存储）
- ✅ 分布式追踪（完整链路）
- ✅ trace_id 关联（问题定位）
- ✅ 日志聚合（高效写入）

**预计工期**：2.5-3.5 周  
**优先级**：高  
**复杂度**：中高

---

**下一步行动**：
1. 创建 flux-logging crate
2. 实现结构化日志
3. 实现日志采样
4. 集成 OpenTelemetry

**规划完成时间**: 2026-02-20
