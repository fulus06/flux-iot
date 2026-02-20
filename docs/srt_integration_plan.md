# SRT 协议集成方案（基于 srt-rs）

**日期**: 2026-02-20  
**策略**: 使用现有 Rust SRT 库而非从头实现  
**目标库**: [srt-rs](https://github.com/russelltg/srt-rs)  
**预计工期**: 1-2 周（相比从头实现节省 3-4 周）

---

## 📊 为什么选择 srt-rs？

### 优势

1. **纯 Rust 实现**
   - 无 unsafe 代码
   - 完整的 Rust 安全保证
   - 与 FLUX IOT 技术栈完美契合

2. **功能完整**
   - ✅ Listen/Connect/Rendezvous 模式
   - ✅ 可靠传输（ARQ）
   - ✅ TsbPd（时间戳播放延迟）
   - ✅ 拥塞控制
   - ✅ AES 加密
   - ✅ 双向传输

3. **高性能**
   - 基于 Tokio 异步运行时
   - 零堆分配设计
   - 线程效率高（相比 libsrt）

4. **活跃维护**
   - 16 个贡献者
   - 5 个发布版本
   - 持续更新

### 对比从头实现

| 维度 | 从头实现 | 使用 srt-rs |
|------|---------|-------------|
| **工期** | 4-6 周 | 1-2 周 |
| **风险** | 高（协议复杂） | 低（成熟库） |
| **兼容性** | 需要测试 | 已验证 |
| **维护成本** | 高 | 低 |
| **功能完整性** | 需要逐步实现 | 开箱即用 |

---

## 🏗️ 集成架构

### 1. 库结构

srt-rs 包含多个 crate：

```
srt-rs/
├── srt-protocol    # 核心协议状态机（无 tokio 依赖）
├── srt-tokio       # Tokio 集成（推荐使用）
├── srt-transmit    # CLI 工具
├── srt-c           # C 绑定（可选）
└── srt-c-unittests # 单元测试
```

**我们将使用**：`srt-tokio`（稳定 API，完整功能）

### 2. 集成方案

```
crates/flux-srt/
├── Cargo.toml              # 添加 srt-tokio 依赖
├── src/
│   ├── main.rs             # HTTP API 服务器（保留）
│   ├── lib.rs              # 库导出
│   ├── listener.rs         # SRT Listener 封装
│   ├── sender.rs           # SRT Sender 封装
│   ├── stream_manager.rs   # 流管理器
│   ├── statistics.rs       # 统计信息
│   └── telemetry.rs        # Telemetry 客户端（保留）
└── tests/
    ├── integration_tests.rs
    └── interop_tests.rs    # 与 FFmpeg/OBS 互操作测试
```

---

## 📋 实施计划

### 阶段 1：依赖集成（2-3 天）

#### 任务 1.1：添加依赖
```toml
[dependencies]
srt-tokio = "0.4"  # 或最新版本
srt-protocol = "0.4"
```

#### 任务 1.2：移除旧代码
- 删除 `src/receiver.rs`（简化实现）
- 删除 `src/sender.rs`（简化实现）
- 保留 `src/main.rs`（HTTP API）
- 保留 `src/telemetry.rs`

#### 任务 1.3：创建新模块
- `src/listener.rs` - 封装 srt-tokio 的 Listener
- `src/sender.rs` - 封装 srt-tokio 的 Sender
- `src/stream_manager.rs` - 流管理逻辑

---

### 阶段 2：Listener 实现（3-4 天）

#### 代码示例

```rust
// src/listener.rs
use srt_tokio::SrtSocket;
use anyhow::Result;
use bytes::Bytes;
use tokio::sync::mpsc;

pub struct SrtListener {
    port: u16,
}

impl SrtListener {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn start(
        self,
        tx: mpsc::Sender<SrtPacket>,
    ) -> Result<()> {
        // 创建 SRT Listener
        let mut listener = SrtSocket::builder()
            .listen_on(self.port)
            .await?;

        tracing::info!("SRT Listener started on port {}", self.port);

        // 接收数据
        while let Some((_instant, bytes)) = listener.try_next().await? {
            let packet = SrtPacket {
                data: bytes,
                timestamp: std::time::Instant::now(),
            };
            
            if tx.send(packet).await.is_err() {
                break;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SrtPacket {
    pub data: Bytes,
    pub timestamp: std::time::Instant,
}
```

#### 任务 2.1：实现 Listener
- 封装 `SrtSocket::builder().listen_on()`
- 处理连接接受
- 数据接收和转发

#### 任务 2.2：集成到 HTTP API
- 修改 `POST /api/v1/srt/streams` 使用新 Listener
- 保持现有 API 兼容性

#### 任务 2.3：测试
- 单元测试
- 与 FFmpeg 互操作测试

---

### 阶段 3：Sender 实现（2-3 天）

#### 代码示例

```rust
// src/sender.rs
use srt_tokio::SrtSocket;
use anyhow::Result;
use bytes::Bytes;

pub struct SrtSender {
    socket: SrtSocket,
}

impl SrtSender {
    pub async fn connect(addr: &str) -> Result<Self> {
        let socket = SrtSocket::builder()
            .call(addr, None)
            .await?;

        tracing::info!("SRT Sender connected to {}", addr);

        Ok(Self { socket })
    }

    pub async fn send(&mut self, data: Bytes) -> Result<()> {
        use futures::SinkExt;
        
        self.socket.send((std::time::Instant::now(), data)).await?;
        Ok(())
    }
}
```

#### 任务 3.1：实现 Sender
- 封装 `SrtSocket::builder().call()`
- 数据发送逻辑

#### 任务 3.2：添加 HTTP API
- `POST /api/v1/srt/send` - 发送数据到远程

#### 任务 3.3：测试
- 单元测试
- 端到端测试

---

### 阶段 4：高级特性（2-3 天）

#### 任务 4.1：统计信息
```rust
// src/statistics.rs
use srt_protocol::statistics::Statistics;

pub struct SrtStatistics {
    stats: Statistics,
}

impl SrtStatistics {
    pub fn get_metrics(&self) -> SrtMetrics {
        SrtMetrics {
            packets_sent: self.stats.packets_sent,
            packets_received: self.stats.packets_received,
            packets_lost: self.stats.packets_lost,
            rtt: self.stats.rtt,
            bandwidth: self.stats.bandwidth,
        }
    }
}
```

#### 任务 4.2：Telemetry 集成
- 上报连接事件
- 上报统计信息
- 上报错误事件

#### 任务 4.3：配置支持
```rust
// 支持 SRT 配置参数
let socket = SrtSocket::builder()
    .latency(Duration::from_millis(120))
    .encryption(16) // AES-128
    .passphrase("secret")
    .listen_on(port)
    .await?;
```

---

### 阶段 5：测试和文档（1-2 天）

#### 任务 5.1：集成测试
- Listener 测试
- Sender 测试
- 端到端测试

#### 任务 5.2：互操作测试
```bash
# 测试与 FFmpeg 互操作
ffmpeg -re -i input.mp4 -c copy -f mpegts "srt://localhost:9000"

# 测试与 OBS 互操作
# OBS -> Settings -> Stream -> Service: Custom
# Server: srt://localhost:9000
```

#### 任务 5.3：文档
- API 文档
- 使用示例
- 配置说明

---

## 🎯 成功标准

### 功能完整性
- ✅ Listener 模式（接收流）
- ✅ Sender 模式（发送流）
- ✅ 可靠传输（自动重传）
- ✅ 低延迟（< 200ms）
- ✅ 加密支持（AES-128/256）
- ✅ 统计信息收集

### 性能指标
- **延迟**：< 200ms（端到端）
- **吞吐量**：> 100 Mbps
- **丢包恢复**：< 1% 丢包率正常工作
- **并发连接**：> 100

### 兼容性
- ✅ 与 FFmpeg 互操作
- ✅ 与 OBS 互操作
- ✅ 与 libsrt 互操作

---

## 📦 依赖更新

### Cargo.toml 修改

```toml
[dependencies]
# SRT 协议支持
srt-tokio = "0.4"
srt-protocol = "0.4"

# 现有依赖（保留）
tokio = { version = "1.35", features = ["full"] }
bytes = "1.5"
anyhow = "1.0"
tracing = "0.1"
# ... 其他依赖
```

---

## 🚀 快速开始（实现后）

### Listener 模式（接收流）

```rust
use flux_srt::SrtListener;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = SrtListener::new(9000);
    
    let (tx, mut rx) = mpsc::channel(100);
    
    tokio::spawn(async move {
        listener.start(tx).await.unwrap();
    });
    
    while let Some(packet) = rx.recv().await {
        println!("Received {} bytes", packet.data.len());
    }
    
    Ok(())
}
```

### Sender 模式（发送流）

```rust
use flux_srt::SrtSender;
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<()> {
    let mut sender = SrtSender::connect("127.0.0.1:9000").await?;
    
    let data = Bytes::from("Hello, SRT!");
    sender.send(data).await?;
    
    Ok(())
}
```

### HTTP API

```bash
# 启动 Listener
curl -X POST http://localhost:8085/api/v1/srt/streams \
  -H "Content-Type: application/json" \
  -d '{"port": 9000, "stream_name": "live"}'

# 查看流列表
curl http://localhost:8085/api/v1/srt/streams

# 使用 FFmpeg 推流
ffmpeg -re -i input.mp4 -c copy -f mpegts "srt://localhost:9000"
```

---

## 📊 进度追踪

| 阶段 | 任务 | 预计时间 | 状态 |
|------|------|---------|------|
| 1 | 依赖集成 | 2-3 天 | ⏳ 待开始 |
| 2 | Listener 实现 | 3-4 天 | ⏳ 待开始 |
| 3 | Sender 实现 | 2-3 天 | ⏳ 待开始 |
| 4 | 高级特性 | 2-3 天 | ⏳ 待开始 |
| 5 | 测试和文档 | 1-2 天 | ⏳ 待开始 |

**总计**：10-15 天（1-2 周）

---

## ⚠️ 注意事项

### 1. srt-rs 状态
- **注意**：README 标注 "NOT PRODUCTION READY"
- **建议**：先在测试环境验证稳定性
- **备选**：如果不稳定，考虑使用 libsrt 的 FFI 绑定

### 2. 版本选择
- 使用最新稳定版本（当前 0.4.x）
- 关注 GitHub Issues 和更新日志
- 测试与 FFmpeg/OBS 的兼容性

### 3. 性能调优
- 根据实际场景调整延迟参数
- 监控内存使用
- 压力测试

---

## 🔄 回退方案

如果 srt-rs 不满足需求，备选方案：

### 方案 A：使用 libsrt FFI 绑定
- 使用官方 C++ 实现
- 通过 FFI 调用
- 稳定但需要处理 unsafe

### 方案 B：继续自研
- 按照 `srt_protocol_plan.md` 实现
- 工期 4-6 周
- 完全掌控但成本高

---

## 🎉 总结

**选择 srt-rs 的优势**：
- ✅ 节省 3-4 周开发时间
- ✅ 功能完整，开箱即用
- ✅ 纯 Rust，安全可靠
- ✅ 活跃维护，社区支持
- ✅ 与 Tokio 完美集成

**预期成果**：
- 1-2 周内完成 SRT 协议集成
- 达到 90-100% 功能完整性
- 与 FFmpeg/OBS 完全兼容
- 生产环境可用

---

**下一步行动**：
1. 添加 srt-tokio 依赖
2. 实现 Listener 封装
3. 集成到 HTTP API
4. 测试与 FFmpeg 互操作

**预计开始时间**：待定  
**负责人**：FLUX IOT Team
