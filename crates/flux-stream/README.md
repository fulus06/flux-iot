# flux-stream

统一流管理包 - 协议无关的流媒体管理和转码触发系统

## 概述

`flux-stream` 提供了一个协议无关的流管理架构，支持多种输入协议（RTMP、RTSP、SRT、WebRTC）和输出协议（HLS、HTTP-FLV、RTMP、RTSP、WebRTC），并实现了智能的自动转码触发机制。

## 核心特性

- ✅ **协议无关** - 统一管理所有协议的流
- ✅ **自动转码触发** - 根据配置条件自动触发转码
- ✅ **多触发条件** - 支持协议切换、客户端数量、客户端类型、网络质量等多种触发条件
- ✅ **灵活配置** - 通过 `flux-config` 包进行配置管理
- ✅ **类型安全** - 使用 Rust 强类型系统保证正确性

## 架构

```
flux-stream/
  ├─ stream.rs          # Stream trait 和核心类型定义
  ├─ context.rs         # 流上下文管理
  ├─ manager.rs         # 统一流管理器
  └─ trigger.rs         # 转码触发检测器
```

## 使用示例

### 1. 创建流管理器

```rust
use flux_stream::StreamManager;
use flux_config::StreamingConfig;

let config = StreamingConfig::default();
let manager = StreamManager::new(config);
```

### 2. 注册流

```rust
use flux_stream::{Stream, Protocol, StreamMode};
use flux_config::TranscodeTrigger;

// 实现 Stream trait 的具体流（例如 RtmpStream）
let stream: Box<dyn Stream> = create_rtmp_stream();

// 注册流，使用自动模式
manager.register_stream(
    stream,
    StreamMode::Auto {
        triggers: vec![TranscodeTrigger::ProtocolSwitch],
    }
).await?;
```

### 3. 请求输出流

```rust
use flux_stream::{ClientInfo, ClientType, Protocol};

let client_info = ClientInfo {
    client_id: "client-001".to_string(),
    client_type: ClientType::WebBrowser,
    preferred_protocol: Protocol::HttpFlv,
    bandwidth_estimate: Some(2000),
    user_agent: Some("Mozilla/5.0".to_string()),
};

// 自动检测是否需要转码
let output = manager.request_output(&stream_id, client_info).await?;
println!("Output URL: {}", output.url);
```

## 转码触发条件

### 1. 协议切换触发

当检测到客户端请求不同协议时自动转码：

```rust
StreamMode::Auto {
    triggers: vec![TranscodeTrigger::ProtocolSwitch],
}
```

### 2. 客户端数量触发

当客户端数量超过阈值时转码：

```rust
StreamMode::Auto {
    triggers: vec![TranscodeTrigger::ClientThreshold { count: 5 }],
}
```

### 3. 客户端类型多样性触发

当检测到不同类型客户端时转码：

```rust
StreamMode::Auto {
    triggers: vec![TranscodeTrigger::ClientVariety],
}
```

### 4. 网络质量差异触发

当客户端网络质量差异超过阈值时转码：

```rust
StreamMode::Auto {
    triggers: vec![
        TranscodeTrigger::NetworkVariance { threshold: 0.5 }
    ],
}
```

### 5. 组合触发条件

支持多个触发条件组合使用：

```rust
StreamMode::Auto {
    triggers: vec![
        TranscodeTrigger::ProtocolSwitch,
        TranscodeTrigger::ClientThreshold { count: 5 },
        TranscodeTrigger::ClientVariety,
    ],
}
```

## Stream Trait

所有协议的流都需要实现 `Stream` trait：

```rust
#[async_trait]
pub trait Stream: Send + Sync {
    fn stream_id(&self) -> &StreamId;
    fn protocol(&self) -> Protocol;
    fn metadata(&self) -> &StreamMetadata;
    fn status(&self) -> StreamStatus;
    
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
}
```

## 集成示例

### RTMP 流实现

```rust
use flux_stream::{Stream, Protocol, StreamMetadata, StreamStatus};
use async_trait::async_trait;

pub struct RtmpStream {
    stream_id: StreamId,
    metadata: StreamMetadata,
    status: StreamStatus,
    // RTMP 特定字段...
}

#[async_trait]
impl Stream for RtmpStream {
    fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }
    
    fn protocol(&self) -> Protocol {
        Protocol::RTMP
    }
    
    fn metadata(&self) -> &StreamMetadata {
        &self.metadata
    }
    
    fn status(&self) -> StreamStatus {
        self.status
    }
    
    async fn start(&mut self) -> Result<()> {
        // 启动 RTMP 流
        self.status = StreamStatus::Running;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        // 停止 RTMP 流
        self.status = StreamStatus::Stopped;
        Ok(())
    }
}
```

## 依赖

- `flux-config` - 配置管理
- `flux-media-core` - 媒体核心类型
- `tokio` - 异步运行时
- `anyhow` - 错误处理
- `async-trait` - 异步 trait 支持

## 测试

运行测试：

```bash
cargo test -p flux-stream
```

所有测试应该通过：
- ✅ 流注册和注销
- ✅ 协议切换触发
- ✅ 客户端数量触发
- ✅ 自动转码触发

## 许可证

与 FLUX IOT 项目相同
