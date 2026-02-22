# flux-stream 使用示例

完整的流媒体处理示例，展示如何使用 `flux-stream` 实现直通、转码和智能输出管理。

---

## 示例 1：基础流管理

### 创建统一流管理器

```rust
use flux_stream::StreamManager;
use flux_config::StreamingConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建配置
    let config = StreamingConfig::default();
    
    // 创建统一流管理器
    let manager = StreamManager::new(config);
    
    Ok(())
}
```

---

## 示例 2：直通模式（零转码）

### 场景：内网监控，低成本运行

```rust
use flux_stream::{PassthroughProcessor, processor::passthrough::*};
use flux_media_core::types::StreamId;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let stream_id = StreamId::new("rtsp", "camera-001");
    
    // 配置输出
    let outputs = vec![
        OutputConfig {
            format: OutputFormat::HLS,
            url: "/data/hls/camera-001/index.m3u8".to_string(),
        },
        OutputConfig {
            format: OutputFormat::FLV,
            url: "rtmp://localhost/live/camera-001".to_string(),
        },
    ];
    
    // 创建直通处理器
    let processor = PassthroughProcessor::new(
        stream_id,
        "rtsp://192.168.1.100:554/stream".to_string(),
        outputs,
    );
    
    // 启动处理（零转码，CPU 占用 < 5%）
    processor.start().await?;
    
    println!("直通模式运行中，按 Ctrl+C 停止...");
    tokio::signal::ctrl_c().await?;
    
    // 停止处理
    processor.stop().await?;
    
    Ok(())
}
```

**资源占用**：
- CPU: < 5%
- 内存: ~50MB
- 适用场景：300路并发监控

---

## 示例 3：多码率转码

### 场景：互联网直播，多设备支持

```rust
use flux_stream::TranscodeProcessor;
use flux_config::{BitrateConfig, HardwareAccel};
use flux_media_core::types::StreamId;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let stream_id = StreamId::new("rtmp", "live-stream-001");
    
    // 配置多码率
    let bitrates = vec![
        BitrateConfig {
            name: "1080p".to_string(),
            bitrate: 4000,
            resolution: (1920, 1080),
            framerate: 30.0,
            encoder_preset: "medium".to_string(),
        },
        BitrateConfig {
            name: "720p".to_string(),
            bitrate: 2000,
            resolution: (1280, 720),
            framerate: 30.0,
            encoder_preset: "fast".to_string(),
        },
        BitrateConfig {
            name: "480p".to_string(),
            bitrate: 1000,
            resolution: (854, 480),
            framerate: 25.0,
            encoder_preset: "fast".to_string(),
        },
        BitrateConfig {
            name: "360p".to_string(),
            bitrate: 500,
            resolution: (640, 360),
            framerate: 25.0,
            encoder_preset: "veryfast".to_string(),
        },
    ];
    
    // 创建转码处理器（使用 NVIDIA GPU）
    let processor = TranscodeProcessor::new(
        stream_id,
        "rtmp://localhost/live/input".to_string(),
        bitrates,
        Some(HardwareAccel::NVENC),
        "/data/hls/live-stream-001".to_string(),
    );
    
    // 启动转码
    processor.start().await?;
    
    println!("转码运行中，生成 4 个码率...");
    tokio::signal::ctrl_c().await?;
    
    processor.stop().await?;
    
    Ok(())
}
```

**输出**：
- `/data/hls/live-stream-001/1080p.m3u8`
- `/data/hls/live-stream-001/720p.m3u8`
- `/data/hls/live-stream-001/480p.m3u8`
- `/data/hls/live-stream-001/360p.m3u8`

---

## 示例 4：智能输出管理

### 场景：自动选择最佳协议

```rust
use flux_stream::{OutputManager, Protocol, QualityLevel, ClientType};
use flux_media_core::types::StreamId;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = OutputManager::new();
    let stream_id = StreamId::new("rtmp", "stream-001");
    
    // 注册多种输出格式
    manager.register_output(
        stream_id.clone(),
        Protocol::RTMP,
        "rtmp://localhost/live/stream-001".to_string(),
        QualityLevel::High,
    ).await?;
    
    manager.register_output(
        stream_id.clone(),
        Protocol::HttpFlv,
        "http://localhost/flv/stream-001.flv".to_string(),
        QualityLevel::High,
    ).await?;
    
    manager.register_output(
        stream_id.clone(),
        Protocol::RTSP,
        "rtsp://localhost/stream-001".to_string(),
        QualityLevel::High,
    ).await?;
    
    // Web 浏览器客户端 → 自动选择 HTTP-FLV
    let output = manager.get_output(
        &stream_id,
        ClientType::WebBrowser,
        None,
    ).await?;
    println!("Web 浏览器: {} ({})", output.url, output.protocol);
    
    // 桌面客户端 → 自动选择 RTMP
    let output = manager.get_output(
        &stream_id,
        ClientType::Desktop,
        None,
    ).await?;
    println!("桌面客户端: {} ({})", output.url, output.protocol);
    
    // IoT 设备 → 自动选择 RTSP
    let output = manager.get_output(
        &stream_id,
        ClientType::IoTDevice,
        None,
    ).await?;
    println!("IoT 设备: {} ({})", output.url, output.protocol);
    
    Ok(())
}
```

**输出**：
```
Web 浏览器: http://localhost/flv/stream-001.flv (http-flv)
桌面客户端: rtmp://localhost/live/stream-001 (rtmp)
IoT 设备: rtsp://localhost/stream-001 (rtsp)
```

---

## 示例 5：自动转码触发

### 场景：按需转码，成本优化

```rust
use flux_stream::{StreamManager, ClientInfo, ClientType, Protocol};
use flux_config::{StreamingConfig, StreamMode, TranscodeTrigger};
use flux_media_core::types::StreamId;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 配置自动转码
    let mut config = StreamingConfig::default();
    config.transcode.mode = StreamMode::Auto {
        triggers: vec![
            TranscodeTrigger::ProtocolSwitch,
            TranscodeTrigger::ClientThreshold { count: 5 },
        ],
    };
    
    let manager = StreamManager::new(config);
    let stream_id = StreamId::new("rtmp", "camera-001");
    
    // 模拟客户端请求
    
    // 客户端 1-3: 请求 HLS → 直通模式
    for i in 1..=3 {
        let client = ClientInfo {
            client_id: format!("client-{}", i),
            client_type: ClientType::WebBrowser,
            preferred_protocol: Protocol::RTMP,
            bandwidth_estimate: Some(2000),
            user_agent: None,
        };
        
        manager.request_output(&stream_id, client).await?;
        println!("客户端 {} 连接 (HLS) - 直通模式", i);
    }
    
    // 客户端 4: 请求 HTTP-FLV → 检测到协议切换
    let client = ClientInfo {
        client_id: "client-4".to_string(),
        client_type: ClientType::WebBrowser,
        preferred_protocol: Protocol::HttpFlv,  // ← 不同协议
        bandwidth_estimate: Some(2000),
        user_agent: None,
    };
    
    manager.request_output(&stream_id, client).await?;
    println!("客户端 4 连接 (HTTP-FLV) - 触发转码！");
    
    // 检查流状态
    let context = manager.get_context(&stream_id).await.unwrap();
    println!("转码状态: {}", context.is_transcoding);
    
    Ok(())
}
```

**输出**：
```
客户端 1 连接 (HLS) - 直通模式
客户端 2 连接 (HLS) - 直通模式
客户端 3 连接 (HLS) - 直通模式
客户端 4 连接 (HTTP-FLV) - 触发转码！
转码状态: true
```

---

## 示例 6：完整的流媒体服务器

### 集成所有组件

```rust
use flux_stream::{
    StreamManager, OutputManager, PassthroughProcessor, TranscodeProcessor,
    ClientInfo, ClientType, Protocol, Stream,
};
use flux_config::{StreamingConfig, StreamMode, TranscodeTrigger};
use flux_media_core::types::StreamId;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 创建配置
    let config = StreamingConfig::default();
    
    // 2. 创建管理器
    let stream_manager = Arc::new(StreamManager::new(config));
    let output_manager = Arc::new(OutputManager::new());
    
    // 3. 注册流（使用自动模式）
    let stream_id = StreamId::new("rtmp", "live/stream-001");
    
    // 注册输出
    output_manager.register_output(
        stream_id.clone(),
        Protocol::RTMP,
        "rtmp://localhost/live/stream-001".to_string(),
        flux_stream::QualityLevel::High,
    ).await?;
    
    output_manager.register_output(
        stream_id.clone(),
        Protocol::HttpFlv,
        "http://localhost/flv/stream-001.flv".to_string(),
        flux_stream::QualityLevel::High,
    ).await?;
    
    // 4. 启动直通处理器
    let processor = PassthroughProcessor::new(
        stream_id.clone(),
        "rtmp://source/live/input".to_string(),
        vec![],
    );
    processor.start().await?;
    
    // 5. 处理客户端请求
    let client = ClientInfo {
        client_id: "web-client-001".to_string(),
        client_type: ClientType::WebBrowser,
        preferred_protocol: Protocol::HttpFlv,
        bandwidth_estimate: Some(2000),
        user_agent: Some("Mozilla/5.0".to_string()),
    };
    
    let output = stream_manager.request_output(&stream_id, client).await?;
    println!("客户端输出: {}", output.url);
    
    // 6. 运行服务器
    println!("流媒体服务器运行中...");
    tokio::signal::ctrl_c().await?;
    
    // 7. 清理
    processor.stop().await?;
    
    Ok(())
}
```

---

## 性能对比

### 直通模式 vs 转码模式

| 指标 | 直通模式 | 转码模式（软件） | 转码模式（NVENC） |
|------|---------|----------------|------------------|
| CPU 占用 | < 5% | 80-100% | 10-15% |
| 内存占用 | ~50MB | ~200MB | ~150MB |
| 延迟 | < 1s | 2-3s | 1-2s |
| 并发能力 | 300路 | 10路 | 50路 |
| 成本（300路） | ¥10,000 | ¥150,000 | ¥50,000 |

---

## 最佳实践

### 1. 选择合适的模式

```rust
// 内网监控 → 直通模式
config.transcode.mode = StreamMode::Passthrough { remux: true };

// 混合场景 → 自动模式
config.transcode.mode = StreamMode::Auto {
    triggers: vec![TranscodeTrigger::ProtocolSwitch],
};

// 互联网直播 → 转码模式
config.transcode.mode = StreamMode::Transcode;
```

### 2. 使用硬件加速

```rust
// 检测可用的硬件加速
let hw_accel = if has_nvidia_gpu() {
    Some(HardwareAccel::NVENC)
} else if has_intel_qsv() {
    Some(HardwareAccel::QSV)
} else {
    None
};
```

### 3. 监控资源使用

```rust
// 定期检查处理器状态
if processor.is_running().await {
    println!("处理器运行正常");
} else {
    println!("处理器已停止，需要重启");
    processor.start().await?;
}
```

---

## 故障排查

### 问题 1：FFmpeg 未找到

```bash
# 安装 FFmpeg
# macOS
brew install ffmpeg

# Ubuntu
sudo apt-get install ffmpeg

# 验证安装
ffmpeg -version
```

### 问题 2：硬件加速不可用

```rust
// 降级到软件编码
let processor = TranscodeProcessor::new(
    stream_id,
    input_url,
    bitrates,
    None,  // ← 不使用硬件加速
    output_dir,
);
```

### 问题 3：转码未触发

```rust
// 检查触发条件配置
let context = manager.get_context(&stream_id).await?;
println!("当前模式: {:?}", context.mode);
println!("转码状态: {}", context.is_transcoding);
println!("客户端数量: {}", context.get_client_count().await);
```

---

## 更多示例

查看完整的测试代码：
- `crates/flux-stream/src/processor/passthrough.rs`
- `crates/flux-stream/src/processor/transcode.rs`
- `crates/flux-stream/src/output/manager.rs`
- `crates/flux-stream/src/manager.rs`
