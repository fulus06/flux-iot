# flux-media-core

协议无关的媒体能力层，为 FLUX IOT 平台提供统一的存储和 snapshot 能力。

## 架构设计

```
┌─────────────────────────────────────────────────────────┐
│           协议接入层 (Protocol Daemons)                  │
│  flux-gb28181d │ flux-rtmpd │ flux-rtspd │ ...         │
└─────────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│              flux-media-core (协议无关)                  │
│  ┌──────────────────┐  ┌──────────────────────────┐    │
│  │  MediaStorage    │  │  SnapshotOrchestrator    │    │
│  │  - put_object    │  │  - process_keyframe      │    │
│  │  - get_object    │  │  - get_snapshot          │    │
│  │  - list_objects  │  │  - mode: auto/keyframe/  │    │
│  │  - cleanup       │  │          decode          │    │
│  └──────────────────┘  └──────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

## 核心组件

### 1. MediaStorage

协议无关的存储接口，支持：
- **put_object**: 存储媒体对象（视频片段、音频等）
- **get_object**: 获取媒体对象
- **list_objects**: 列出指定时间范围的对象
- **cleanup**: 清理过期数据

**实现**：
- `FileSystemStorage`: 基于文件系统的存储实现

### 2. SnapshotOrchestrator

统一的 snapshot 编排器，支持三种模式：

#### Auto 模式（推荐）
优先使用 keyframe 快照，失败时自动降级到 decode 快照。

#### Keyframe 模式
- **优点**: 低延迟、低成本、无需解码
- **缺点**: 无法缩放、无法添加水印/OSD
- **适用场景**: 实时预览、快速查看

#### Decode 模式
- **优点**: 高质量、可缩放、可添加水印/OSD
- **缺点**: 需要解码器、延迟较高
- **适用场景**: 高质量截图、需要后处理的场景

### 3. StreamId

协议无关的流标识符，格式：`{protocol}/{identifier}`

**示例**：
- GB28181: `gb28181/34020000001320000001/34020000001320000001`
- RTMP: `rtmp/live/stream123`
- RTSP: `rtsp/192.168.1.100/channel1`

## 使用示例

### 基础存储

```rust
use flux_media_core::{
    storage::{filesystem::FileSystemStorage, MediaStorage, StorageConfig},
    types::StreamId,
};
use bytes::Bytes;
use chrono::Utc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = StorageConfig {
        root_dir: "/var/lib/flux-media".into(),
        retention_days: 7,
        segment_duration_secs: 60,
    };

    let mut storage = FileSystemStorage::new(config)?;
    let stream_id = StreamId::new("gb28181", "device1/channel1");
    
    // 存储数据
    storage.put_object(
        &stream_id,
        Utc::now(),
        Bytes::from("video data"),
    ).await?;

    Ok(())
}
```

### Snapshot 编排

```rust
use flux_media_core::{
    snapshot::{SnapshotOrchestrator, SnapshotRequest, SnapshotMode},
    types::StreamId,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let orchestrator = SnapshotOrchestrator::new(
        PathBuf::from("/var/lib/flux-media/keyframes")
    );

    let stream_id = StreamId::new("gb28181", "device1/channel1");
    
    // 处理视频帧并提取关键帧
    let h264_data = vec![/* H264 数据 */];
    orchestrator.process_keyframe(
        &stream_id,
        &h264_data,
        chrono::Utc::now(),
    ).await?;

    // 获取 snapshot（自动模式）
    let req = SnapshotRequest {
        stream_id: stream_id.clone(),
        mode: SnapshotMode::Auto,
        width: Some(640),
        height: Some(480),
    };

    let snapshot = orchestrator.get_snapshot(req).await?;
    println!("Snapshot mode: {:?}", snapshot.mode_used);
    println!("Snapshot size: {} bytes", snapshot.data.len());

    Ok(())
}
```

### 在协议层集成

```rust
use flux_media_core::{
    storage::{filesystem::FileSystemStorage, MediaStorage},
    snapshot::SnapshotOrchestrator,
    types::{StreamId, VideoSample},
};

struct Gb28181StreamProcessor {
    stream_id: StreamId,
    storage: Arc<RwLock<FileSystemStorage>>,
    orchestrator: Arc<SnapshotOrchestrator>,
}

impl Gb28181StreamProcessor {
    async fn process_rtp_packet(&self, packet: RtpPacket) {
        // 1. 解复用 PS -> H264
        let h264_data = demux_ps(packet.payload());
        
        // 2. 存储原始数据
        let mut storage = self.storage.write().await;
        storage.put_object(
            &self.stream_id,
            chrono::Utc::now(),
            h264_data.clone(),
        ).await.ok();
        
        // 3. 提取关键帧
        self.orchestrator.process_keyframe(
            &self.stream_id,
            &h264_data,
            chrono::Utc::now(),
        ).await.ok();
    }
}
```

## 扩展点

### 自定义解码器

实现 `SnapshotDecoder` trait 以支持自定义解码逻辑：

```rust
use flux_media_core::snapshot::SnapshotDecoder;
use async_trait::async_trait;

struct FfmpegDecoder {
    // ffmpeg 配置
}

#[async_trait]
impl SnapshotDecoder for FfmpegDecoder {
    async fn decode(
        &self,
        h264_data: &[u8],
        width: Option<u32>,
        height: Option<u32>,
    ) -> flux_media_core::Result<Bytes> {
        // 使用 ffmpeg 解码 H264 并生成 JPEG
        todo!("实现 ffmpeg 解码逻辑")
    }
}

// 使用自定义解码器
let orchestrator = SnapshotOrchestrator::new(keyframe_dir)
    .with_decoder(Arc::new(FfmpegDecoder::new()));
```

### 自定义存储后端

实现 `MediaStorage` trait 以支持其他存储后端（如 S3、OSS 等）：

```rust
use flux_media_core::storage::MediaStorage;
use async_trait::async_trait;

struct S3Storage {
    // S3 配置
}

#[async_trait]
impl MediaStorage for S3Storage {
    async fn put_object(
        &mut self,
        stream_id: &StreamId,
        timestamp: DateTime<Utc>,
        data: Bytes,
    ) -> flux_media_core::Result<()> {
        // 实现 S3 上传逻辑
        todo!()
    }
    
    // ... 实现其他方法
}
```

## 测试

```bash
# 运行所有测试
cargo test -p flux-media-core

# 运行特定测试
cargo test -p flux-media-core test_snapshot_orchestrator_keyframe
```

## 目录结构

```
/var/lib/flux-media/
├── gb28181/
│   └── device1/
│       └── channel1/
│           ├── segments/
│           │   ├── 1234567890000.bin
│           │   └── 1234567891000.bin
│           └── keyframes/
│               ├── 1234567890000.h264
│               └── 1234567891000.h264
└── rtmp/
    └── live/
        └── stream123/
            ├── segments/
            └── keyframes/
```

## 性能考虑

1. **异步 I/O**: 所有存储操作均为异步，避免阻塞
2. **缓存**: SnapshotOrchestrator 内置缓存，避免重复读取
3. **零拷贝**: 使用 `Bytes` 类型减少内存拷贝
4. **并发安全**: 使用 `RwLock` 保证多线程安全

## 许可证

与 FLUX IOT 项目保持一致
