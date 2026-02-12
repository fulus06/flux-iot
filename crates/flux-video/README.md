# flux-video

FLUX IOT 视频流监控核心库 - 极致轻量、高并发的视频流处理系统

## 特性

- ✅ **极致轻量**：单节点模式仅 40-80MB 内存
- ✅ **高并发**：单节点支持 100+ 路摄像头
- ✅ **多协议**：RTSP、RTMP、GB28181、WebRTC
- ✅ **零解码关键帧提取**：CPU 占用 < 1%
- ✅ **灵活存储**：本地/NAS/NVR/云存储
- ✅ **自动重连**：指数退避重连机制

## 快速开始

### 1. 启动 HTTP 服务器

```bash
cargo run --example video_server
```

服务器将在 `http://localhost:8080` 启动。

### 2. HTTP API 使用

#### 健康检查

```bash
curl http://localhost:8080/health
```

#### 创建 RTSP 流

```bash
curl -X POST http://localhost:8080/api/video/streams \
  -H "Content-Type: application/json" \
  -d '{
    "stream_id": "camera_001",
    "protocol": "rtsp",
    "url": "rtsp://username:password@192.168.1.100:554/stream"
  }'
```

#### 列出所有流

```bash
curl http://localhost:8080/api/video/streams
```

#### 获取流信息

```bash
curl http://localhost:8080/api/video/streams/camera_001
```

#### 获取快照

```bash
curl http://localhost:8080/api/video/streams/camera_001/snapshot
```

#### 删除流

```bash
curl -X DELETE http://localhost:8080/api/video/streams/camera_001
```

### 3. 自动化集成测试

```bash
# 运行所有测试
cargo test -p flux-video

# 运行集成测试
cargo test -p flux-video --test integration_test

# 运行完整流水线演示
cargo run --example full_pipeline_demo
```

### 4. 人工验证测试（推荐）🎯

**一键启动完整测试环境**：

```bash
# 启动：屏幕捕获 → RTSP推流 → flux-video → Web播放器
./start_manual_test.sh
```

浏览器会自动打开 Web 播放器，您可以：
- 👁️ 实时观看推流状态
- 📊 查看统计数据（帧数、关键帧、数据量）
- 📝 监控日志输出
- 📸 测试截图功能

**手动步骤**（如果需要）：

```bash
# 终端 1: 启动服务器
cargo run --example video_server

# 终端 2: 启动推流器
cargo run --example screen_capture_streamer

# 浏览器: 打开播放器
open http://localhost:8080/player.html?stream=screen_capture
```

详细指南：[docs/MANUAL_TEST_GUIDE.md](docs/MANUAL_TEST_GUIDE.md)

## 架构

```
flux-video/
├── engine.rs           # 流媒体引擎
├── stream/             # 协议层
│   └── rtsp.rs        # RTSP 支持
├── codec/             # 编解码（零解码）
│   └── mod.rs         # H.264 NALU 解析
├── storage/           # 存储层
│   ├── standalone.rs  # 单节点模式
│   ├── pipeline/      # 写入流水线
│   └── backend/       # 多后端支持
└── snapshot/          # 关键帧提取
    └── mod.rs         # 零解码提取器
```

## 示例

### 基础使用

```rust
use flux_video::{
    engine::VideoEngine,
    stream::RtspStream,
    snapshot::KeyframeExtractor,
};

#[tokio::main]
async fn main() {
    // 创建引擎
    let engine = VideoEngine::new();
    
    // 创建 RTSP 流
    let mut stream = RtspStream::new(
        "camera_001".to_string(),
        "rtsp://192.168.1.100:554/stream".to_string(),
    );
    
    // 启动流
    stream.start().await.unwrap();
    
    // 接收媒体包
    while let Some(packet) = stream.recv().await {
        println!("Received: {} bytes", packet.data.len());
    }
}
```

### 关键帧提取

```rust
use flux_video::snapshot::KeyframeExtractor;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let mut extractor = KeyframeExtractor::new(PathBuf::from("./keyframes"))
        .with_interval(5); // 每 5 秒提取一次
    
    // 处理视频数据
    let keyframe = extractor.process(
        "camera_001",
        &video_data,
        chrono::Utc::now()
    ).await.unwrap();
    
    if let Some(kf) = keyframe {
        println!("Keyframe saved: {}", kf.file_path);
    }
}
```

## 性能指标

| 指标 | 单节点模式 |
|------|-----------|
| **并发能力** | 100+ 路 |
| **内存占用** | < 256 MB |
| **CPU 占用** | < 30% |
| **I/O 吞吐** | > 200 MB/s |
| **关键帧提取** | > 1000 帧/秒 |

## 开发状态

- ✅ M1.1: 基础架构
- ✅ M1.2: 存储层单节点模式
- ✅ M1.3: RTSP 协议支持
- ✅ M1.4: 关键帧提取（零解码）
- ✅ M1.5: HTTP API 和集成测试

**Milestone 1 完成！** 🎉

## 许可证

MIT License
