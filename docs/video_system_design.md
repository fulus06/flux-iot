# FLUX IOT 视频流监控系统设计方案

**版本**: v1.0  
**日期**: 2026年02月11日  
**状态**: 设计阶段

---

## 目录

1. [系统概述](#系统概述)
2. [架构设计](#架构设计)
3. [核心模块](#核心模块)
4. [GB28181 协议实现](#gb28181-协议实现)
5. [存储策略](#存储策略)
6. [性能优化](#性能优化)
7. [实施路线图](#实施路线图)
8. [技术栈](#技术栈)

---

## 系统概述

### 设计目标

为 FLUX IOT 平台增加视频流监控能力，支持：

- ✅ **多协议接入**: RTSP、RTMP、GB28181、WebRTC
- ✅ **视频录制**: 支持分片。支持本地、NAS、NVR、云存储等多种后端
- ✅ **关键帧提取**: 智能保存关键帧，节省存储空间
- ✅ **AI 识别**: 集成云厂商 API 进行危险检测
- ✅ **云台控制**: 支持 GB28181 PTZ 控制
- ✅ **历史回放**: 支持时间范围查询和倍速播放

### 核心特性

| 特性 | 说明 |
|------|------|
| **高性能** | 零拷贝转发、Worker Pool、硬件加速 |
| **可扩展** | Native 插件架构，支持自定义协议 |
| **灵活存储** | 多后端支持，主备/多副本/分层策略 |
| **安全隔离** | 插件沙箱，故障不影响主系统 |
| **易集成** | RESTful API + Rhai 脚本 |

---

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────┐
│  FLUX IOT Core                                          │
│  ├─ flux-core (业务逻辑)                                │
│  ├─ flux-mqtt (设备接入)                                │
│  ├─ flux-script (Rhai 引擎)                             │
│  └─ flux-plugin (统一插件管理)                          │
│      ├─ Wasm Plugins (轻量级逻辑)                       │
│      └─ Native Plugins (视频处理)                       │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  flux-video (视频核心)                                  │
│  ├─ VideoEngine (流管理)                                │
│  ├─ StreamWorkerPool (性能优化)                         │
│  ├─ Protocol Adapters                                   │
│  │   ├─ RTSP (Native Plugin)                            │
│  │   ├─ RTMP (Native Plugin)                            │
│  │   └─ GB28181 (Native Plugin)                         │
│  ├─ Storage (多后端)                                    │
│  │   ├─ Local (本地文件系统)                            │
│  │   ├─ NAS (网络存储)                                  │
│  │   ├─ NVR (录像机服务器)                              │
│  │   └─ Cloud (云存储)                                  │
│  └─ KeyframeExtractor (关键帧提取)                      │
└─────────────────────────────────────────────────────────┘
```

### 插件架构

#### 双插件系统

```
FLUX IOT 插件体系
├── Wasm 插件 (现有)
│   └── 用途：轻量级业务逻辑、协议转换、数据处理
│
└── Native 插件 (新增)
    └── 用途：视频流处理、编解码、AI 推理等重计算任务
```

#### 统一插件管理器

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn plugin_type(&self) -> PluginType;
    fn init(&mut self, config: &Config) -> Result<()>;
}

pub enum PluginType {
    Wasm(WasmPlugin),      // 轻量级逻辑
    Native(NativePlugin),  // 重计算任务
}

pub struct UnifiedPluginManager {
    wasm_host: WasmHost,
    native_loader: NativeLoader,
}
```

**优势**：
- 开发者无需关心插件类型
- 统一的配置和管理
- 降低学习成本

---

## 核心模块

### 1. flux-video Crate 结构

```
crates/flux-video/
├── src/
│   ├── lib.rs              # 公共 API
│   ├── engine.rs           # 流媒体引擎核心
│   ├── stream/
│   │   ├── mod.rs          # 流抽象
│   │   ├── rtsp.rs         # RTSP 协议
│   │   ├── rtmp.rs         # RTMP 协议
│   │   ├── gb28181.rs      # GB28181 协议
│   │   └── webrtc.rs       # WebRTC 协议
│   ├── codec/
│   │   ├── h264.rs         # H.264 编解码
│   │   ├── h265.rs         # H.265 编解码
│   │   └── aac.rs          # AAC 音频
│   ├── storage/
│   │   ├── mod.rs          # 存储抽象层
│   │   ├── local.rs        # 本地文件系统
│   │   ├── nvr.rs          # 录像机服务器
│   │   ├── nas.rs          # NAS/存储服务器
│   │   └── cloud.rs        # 云存储（S3/OSS）
│   ├── snapshot/
│   │   ├── extractor.rs    # 帧提取
│   │   └── storage.rs      # 关键帧存储
│   └── ai/
│       ├── inference.rs    # 本地推理（ONNX）
│       └── cloud_api.rs    # 云厂商 API
└── Cargo.toml
```

### 2. 视频流引擎

```rust
use dashmap::DashMap;
use tokio::sync::broadcast;

/// 流媒体引擎：管理所有活跃流
pub struct VideoEngine {
    // 使用 DashMap 实现无锁并发访问
    streams: DashMap<String, Arc<dyn VideoStream>>,
    
    // 全局事件总线
    event_bus: broadcast::Sender<StreamEvent>,
    
    // Worker Pool（性能优化）
    worker_pool: StreamWorkerPool,
}

impl VideoEngine {
    /// 发布流（由协议插件调用）
    pub fn publish_stream(&self, stream: Arc<dyn VideoStream>) -> Result<()>;
    
    /// 订阅流（由消费者调用）
    pub fn subscribe_stream(&self, stream_id: &str) -> Result<mpsc::Receiver<MediaPacket>>;
    
    /// 获取所有活跃流
    pub fn list_streams(&self) -> Vec<StreamInfo>;
}
```

### 3. 流抽象层

```rust
/// 核心抽象：统一的流接口
pub trait VideoStream: Send + Sync {
    fn stream_id(&self) -> &str;
    fn video_track(&self) -> Option<Arc<VideoTrack>>;
    fn audio_track(&self) -> Option<Arc<AudioTrack>>;
    
    // 订阅者模式：零拷贝转发
    fn subscribe(&self) -> mpsc::Receiver<MediaPacket>;
    fn publish(&self, packet: MediaPacket) -> Result<()>;
}

/// 媒体数据包（零拷贝）
pub struct MediaPacket {
    pub data: Arc<Bytes>,  // 零拷贝
    pub timestamp: Duration,
    pub is_keyframe: bool,
    pub codec: Codec,
}
```

---

## GB28181 协议实现

### 协议层次

```
┌─────────────────────────────────────────┐
│  应用层：设备管理、目录查询、云台控制    │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│  信令层：SIP (RFC 3261)                  │
│  ├─ REGISTER (设备注册)                  │
│  ├─ MESSAGE (目录、状态、报警)           │
│  ├─ INVITE (实时/回放请求)               │
│  ├─ ACK/BYE (会话确认/结束)              │
│  └─ INFO (云台控制)                      │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│  会话描述：SDP (RFC 4566)                │
│  ├─ 媒体格式 (H.264/H.265/G.711)         │
│  ├─ RTP 端口                             │
│  └─ 传输协议 (RTP/UDP)                   │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│  媒体传输：RTP/RTCP (RFC 3550)           │
│  ├─ RTP: 实时数据传输                    │
│  └─ RTCP: 控制与统计                     │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│  媒体封装：PS 流 (ISO/IEC 13818-1)       │
│  ├─ PS Header                            │
│  ├─ System Header                        │
│  ├─ Program Stream Map                   │
│  └─ PES Packets (H.264/H.265 NALU)       │
└─────────────────────────────────────────┘
```

### 核心模块

#### 1. SIP 信令模块

```rust
pub struct SipClient {
    local_addr: SocketAddr,
    server_addr: SocketAddr,
    device_id: String,
    domain: String,
    password: String,
    socket: Arc<UdpSocket>,
}

impl SipClient {
    /// 设备注册（支持摘要认证）
    pub async fn register(&self) -> Result<()>;
    
    /// 发起实时视频请求
    pub async fn invite_live(&self, channel_id: &str) -> Result<SdpSession>;
    
    /// 历史回放
    pub async fn playback(&self, channel_id: &str, start: DateTime, end: DateTime) -> Result<SdpSession>;
    
    /// 云台控制
    pub async fn ptz_control(&self, channel_id: &str, command: PtzCommand) -> Result<()>;
}
```

**关键实现**：
- 摘要认证：`response = MD5(MD5(username:realm:password):nonce:MD5(method:uri))`
- Call-ID 生成：UUID v4
- CSeq 序列号管理

#### 2. RTP 接收模块

```rust
pub struct RtpReceiver {
    socket: UdpSocket,
    ssrc: Option<u32>,
    sequence_number: u16,
}

impl RtpReceiver {
    /// 接收 RTP 包
    pub async fn recv_packet(&mut self) -> Result<RtpPacket>;
    
    /// 解析 RTP 头（12 字节固定头）
    fn parse_rtp_packet(&self, data: &[u8]) -> Result<RtpPacket>;
}

pub struct RtpPacket {
    pub version: u8,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: Bytes,  // PS 流片段
}
```

#### 3. PS 流解封装模块

```rust
pub struct PsDemuxer {
    buffer: BytesMut,
    video_stream_id: Option<u8>,
    audio_stream_id: Option<u8>,
}

impl PsDemuxer {
    /// 输入 RTP Payload（PS 流片段）
    pub fn input(&mut self, data: Bytes) -> Result<()>;
    
    /// 解析 PS 包，提取 H.264/H.265 帧
    pub fn demux(&mut self) -> Result<Vec<MediaFrame>>;
}
```

**解析流程**：
1. 查找起始码（0x000001BA/BB/BC/E0-EF）
2. 解析 Pack Header（SCR、mux_rate）
3. 解析 PES 包（提取 PTS/DTS）
4. 提取 ES 数据（H.264 NALU）

#### 4. 完整流程

```rust
pub struct Gb28181Client {
    sip_client: Arc<SipClient>,
    rtp_receiver: Option<RtpReceiver>,
    ps_demuxer: PsDemuxer,
}

impl Gb28181Client {
    pub async fn start_live_stream(&mut self, channel_id: &str) -> Result<mpsc::Receiver<MediaFrame>> {
        // 1. 发送 INVITE 请求
        let sdp = self.sip_client.invite_live(channel_id).await?;
        
        // 2. 创建 RTP 接收器
        let mut rtp_receiver = RtpReceiver::new(sdp.media_port).await?;
        
        // 3. 启动接收任务
        tokio::spawn(async move {
            let mut demuxer = PsDemuxer::new();
            loop {
                let rtp_packet = rtp_receiver.recv_packet().await?;
                demuxer.input(rtp_packet.payload)?;
                let frames = demuxer.demux()?;
                // 发送到订阅者
            }
        });
        
        Ok(rx)
    }
}
```

---

## 存储策略

### 存储抽象层

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 保存视频分片
    async fn save_segment(
        &self,
        stream_id: &str,
        timestamp: DateTime<Utc>,
        data: Bytes,
        metadata: SegmentMetadata,
    ) -> Result<String>;
    
    /// 保存关键帧
    async fn save_keyframe(
        &self,
        stream_id: &str,
        timestamp: DateTime<Utc>,
        frame_data: Bytes,
    ) -> Result<String>;
    
    /// 查询视频分片
    async fn query_segments(
        &self,
        stream_id: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<SegmentInfo>>;
    
    /// 删除过期数据
    async fn cleanup_expired(&self, before: DateTime<Utc>) -> Result<usize>;
}
```

### 多后端支持

#### 1. 本地存储

```rust
pub struct LocalStorage {
    base_path: PathBuf,
    retention_days: u32,
}
```

**目录结构**：
```
base_path/
├── stream_id_1/
│   ├── 2026-02-11/
│   │   ├── 10/
│   │   │   ├── 1707624000.mp4
│   │   │   └── 1707624000.json (metadata)
│   │   └── 11/
│   └── keyframes/
│       └── 2026-02-11/
│           ├── 1707624000123.jpg
│           └── 1707624005456.jpg
```

#### 2. NAS 存储

```rust
pub struct NasStorage {
    mount_point: PathBuf,  // 如 /mnt/nas
    retention_days: u32,
}
```

**特点**：
- 通过 NFS/SMB 挂载
- 与本地存储 API 一致
- 支持 rsync 同步

#### 3. NVR 录像机

```rust
pub struct NvrStorage {
    client: Client,
    nvr_url: String,
    auth_token: String,
}
```

**接口**：
- `POST /api/v1/recordings/{stream_id}` - 上传录像
- `GET /api/v1/recordings/{stream_id}?start=&end=` - 查询录像
- `GET /api/v1/recordings/{id}/playback` - 回放 URL

#### 4. 云存储

```rust
pub struct CloudStorage {
    store: Box<dyn ObjectStore>,  // S3/OSS/GCS
    bucket: String,
}
```

**支持**：
- AWS S3
- 阿里云 OSS
- 腾讯云 COS
- MinIO

### 存储策略

#### Phase 1：主备模式（MVP）

```rust
pub struct VideoStorage {
    primary: StorageBackend,
    backup: Option<StorageBackend>,
}

impl VideoStorage {
    pub async fn save(&self, segment: VideoSegment) -> Result<String> {
        match self.primary.save(segment.clone()).await {
            Ok(url) => Ok(url),
            Err(e) if self.backup.is_some() => {
                self.backup.as_ref().unwrap().save(segment).await
            }
            Err(e) => Err(e),
        }
    }
}
```

#### Phase 2：分层存储（高级）

```rust
pub enum StoragePolicy {
    /// 主备模式
    PrimaryBackup { primary: usize, backup: usize },
    
    /// 多副本模式
    MultiReplica { replicas: Vec<usize> },
    
    /// 分层存储
    Tiered {
        hot_storage: usize,    // 本地 SSD (7天)
        warm_storage: usize,   // NAS (30天)
        cold_storage: usize,   // 云存储 (90天)
        hot_days: u32,
        warm_days: u32,
    },
}
```

**数据迁移**：
```
热数据 (7天)  → 本地 SSD (快速访问)
    ↓ 定时任务
温数据 (30天) → NAS (大容量)
    ↓ 定时任务
冷数据 (90天) → 云存储 (归档)
```

---

## 性能优化

### 1. Worker Pool 模式

**问题**：每个流独立 Task 导致调度开销

**方案**：

```rust
pub struct StreamWorkerPool {
    workers: Vec<StreamWorker>,
    task_queue: Arc<SegQueue<StreamTask>>,
}

impl StreamWorkerPool {
    pub fn new(worker_count: usize) -> Self {
        // 创建固定数量的 Worker
        // 每个 Worker 处理多个流
    }
    
    pub fn submit(&self, stream: Arc<dyn VideoStream>) {
        self.task_queue.push(StreamTask::Process(stream));
    }
}
```

**收益**：
- 减少 Task 数量（100 个流 → 8 个 Worker）
- 更好的 CPU 缓存局部性
- 支持优先级调度

### 2. 零解码关键帧提取

**问题**：解码 H.264 消耗 CPU

**方案**：

```rust
use h264_reader::nal::{Nal, RefNal};

pub struct KeyframeExtractor {}

impl KeyframeExtractor {
    /// 直接解析 NALU，无需解码
    pub fn extract_idr_frame(&self, h264_data: &[u8]) -> Option<Vec<u8>> {
        for nal in h264_reader::nal::iterate(h264_data) {
            if nal.nal_unit_type() == UnitType::SliceLayerWithoutPartitioningIdr {
                return Some(nal.as_bytes().to_vec());
            }
        }
        None
    }
    
    /// 如需缩略图，使用硬件加速
    pub async fn generate_thumbnail(&self, idr_frame: &[u8]) -> Result<Vec<u8>> {
        #[cfg(target_os = "linux")]
        if let Ok(thumbnail) = self.hw_decode_thumbnail(idr_frame).await {
            return Ok(thumbnail);
        }
        
        self.sw_decode_thumbnail(idr_frame).await
    }
}
```

**收益**：
- 关键帧提取性能提升 10x+
- 降低 CPU 使用率
- 支持硬件加速（VAAPI/NVDEC）

### 3. 存储批量写入

**问题**：多副本存储放大 I/O 压力

**方案**：

```rust
pub struct BufferedStorage {
    backend: Arc<dyn StorageBackend>,
    buffer: Arc<Mutex<Vec<VideoSegment>>>,
    flush_interval: Duration,
}

impl BufferedStorage {
    pub async fn save(&self, segment: VideoSegment) -> Result<()> {
        self.buffer.lock().await.push(segment);
        Ok(())
    }
    
    async fn flush_worker(&self) {
        loop {
            tokio::time::sleep(self.flush_interval).await;
            let segments = std::mem::take(&mut *self.buffer.lock().await);
            if !segments.is_empty() {
                self.backend.save_batch(segments).await.ok();
            }
        }
    }
}
```

**收益**：
- 减少磁盘 I/O 次数
- 提升吞吐量
- 降低延迟抖动

### 4. 零拷贝转发

```rust
pub struct MediaPacket {
    pub data: Arc<Bytes>,  // 使用 Arc 共享，避免拷贝
    pub timestamp: Duration,
    pub is_keyframe: bool,
}

// 订阅者直接共享同一份内存
impl VideoStream for RtspStream {
    fn subscribe(&self) -> mpsc::Receiver<MediaPacket> {
        let (tx, rx) = mpsc::channel(100);
        let packet = self.current_packet.clone(); // 仅克隆 Arc
        tx.send(packet).await.ok();
        rx
    }
}
```

---

## 实施路线图

### Milestone 1：核心能力（2 周）

**目标**：可用的 RTSP 监控系统

| 任务 | 工期 | 产出 |
|------|------|------|
| Native 插件框架 | 3 天 | `NativePluginManager` |
| `flux-video` 核心引擎 | 3 天 | `VideoEngine` + 流抽象 |
| RTSP 协议支持 | 3 天 | RTSP Native 插件 |
| 本地存储 | 2 天 | `LocalStorage` |
| 关键帧提取（零解码） | 2 天 | `KeyframeExtractor` |
| HTTP API | 1 天 | RESTful API |

**验收标准**：
- ✅ 能接入 RTSP 摄像头
- ✅ 能录制视频到本地
- ✅ 能提取关键帧
- ✅ 提供 HTTP API

---

### Milestone 2：协议扩展（2 周）

**目标**：支持国标设备的完整监控平台

| 任务 | 工期 | 产出 |
|------|------|------|
| GB28181 SIP 信令 | 5 天 | `SipClient` |
| GB28181 RTP 接收 | 2 天 | `RtpReceiver` |
| GB28181 PS 解封装 | 5 天 | `PsDemuxer` |
| NVR/NAS 存储后端 | 2 天 | `NvrStorage` + `NasStorage` |
| 录像回放 API | 1 天 | 回放接口 |

**验收标准**：
- ✅ 能接入 GB28181 设备
- ✅ 能控制云台
- ✅ 能查询历史录像
- ✅ 支持多存储后端

---

### Milestone 3：高级特性（2-4 周）

**目标**：生产级视频监控平台

| 任务 | 工期 | 产出 |
|------|------|------|
| 分层存储策略 | 3 天 | `StorageStrategy` |
| AI 危险识别 | 3 天 | `CloudVisionClient` |
| Worker Pool 优化 | 3 天 | `StreamWorkerPool` |
| 硬件加速 | 5 天 | VAAPI/NVDEC 支持 |
| Rhai 脚本集成 | 2 天 | 视频 API 封装 |
| Web UI | 5 天 | 管理界面 |

**验收标准**：
- ✅ 支持大规模并发（100+ 流）
- ✅ AI 识别准确率 > 90%
- ✅ 存储成本降低 50%
- ✅ 完整的 Web 管理界面

---

## 技术栈

### 核心依赖

```toml
[dependencies]
# 异步运行时
tokio = { version = "1.35", features = ["full"] }

# 零拷贝内存管理
bytes = "1.5"
arc-swap = "1.6"

# RTSP 客户端
retina = "0.4"

# H.264 解析
h264-reader = "0.7"

# RTP/SDP
rtp = "0.6"
sdp = "0.5"

# 对象存储
object_store = { version = "0.9", features = ["aws", "gcp"] }

# HTTP 客户端
reqwest = { version = "0.11", features = ["json"] }

# 图像处理
image = "0.24"

# 并发数据结构
dashmap = "5.5"
crossbeam = "0.8"

# 日志
tracing = "0.1"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 时间处理
chrono = "0.4"

# 加密（摘要认证）
md5 = "0.7"

# UUID
uuid = { version = "1.0", features = ["v4"] }

# Native 插件加载
libloading = "0.8"

# 异步特征
async-trait = "0.1"
```

### 可选依赖

```toml
[dependencies]
# 硬件加速（Linux）
ffmpeg-next = { version = "6.0", optional = true }
va-api = { version = "0.1", optional = true }

# WebRTC 支持
webrtc = { version = "0.9", optional = true }

# ONNX 推理
ort = { version = "1.16", optional = true }
```

---

## 配置示例

```toml
# config.toml
[video]
enabled = true
worker_count = 8  # Worker Pool 大小

# 存储策略
[video.storage]
policy = "primary_backup"  # "primary_backup" | "multi_replica" | "tiered"

# 本地存储（主）
[[video.storage.backends]]
type = "local"
name = "local_ssd"
base_path = "/data/video/hot"
retention_days = 7

# NAS 存储（备）
[[video.storage.backends]]
type = "nas"
name = "nas_storage"
mount_point = "/mnt/nas/video"
retention_days = 30

# 关键帧存储
[video.keyframe]
enabled = true
interval_seconds = 5
storage_backend = "local_ssd"
retention_days = 90

# GB28181 配置
[video.gb28181]
enabled = true
sip_server = "192.168.1.10:5060"
local_ip = "192.168.1.100"
local_port = 5060
domain = "3402000000"
device_id = "34020000001320000001"
password = "12345678"

# 协议支持
[video.protocols]
rtsp = true
rtmp = true
gb28181 = true
webrtc = false
```

---

## HTTP API 设计

### 流管理

```http
# 创建流
POST /api/video/streams
{
  "stream_id": "camera_001",
  "protocol": "rtsp",
  "source_url": "rtsp://192.168.1.100:554/stream"
}

# 列出所有流
GET /api/video/streams

# 获取流信息
GET /api/video/streams/{stream_id}

# 停止流
DELETE /api/video/streams/{stream_id}

# 截图
GET /api/video/streams/{stream_id}/snapshot
```

### 录像管理

```http
# 查询录像
GET /api/video/recordings/{stream_id}?start=2026-02-11T00:00:00Z&end=2026-02-11T23:59:59Z

# 回放
GET /api/video/recordings/{stream_id}/playback?start=...&end=...&speed=1.0
```

### GB28181 特有

```http
# 列出设备
GET /api/video/gb28181/devices

# 云台控制
POST /api/video/gb28181/devices/{device_id}/ptz
{
  "command": "up",
  "speed": 128
}

# 历史回放
POST /api/video/gb28181/devices/{device_id}/playback
{
  "start_time": "2026-02-11T10:00:00Z",
  "end_time": "2026-02-11T11:00:00Z"
}
```

### AI 分析

```http
# 危险检测
POST /api/video/analyze/{stream_id}
{
  "features": ["fire_detection", "person_detection"]
}
```

---

## Rhai 脚本集成

### API 封装

```rust
pub fn register_video_api(engine: &mut rhai::Engine, video_engine: Arc<VideoEngine>) {
    engine.register_async_fn("start_rtsp_stream", |stream_id: String, url: String| async {
        video_engine.start_rtsp_stream(&stream_id, &url).await.is_ok()
    });
    
    engine.register_async_fn("capture_snapshot", |stream_id: String| async {
        video_engine.capture_snapshot(&stream_id).await.ok()
    });
    
    engine.register_async_fn("detect_danger", |stream_id: String| async {
        video_engine.analyze_danger(&stream_id).await.ok()
    });
}
```

### 脚本示例

```rhai
// video_monitor.rhai

// 监控流上线事件
fn on_stream_published(stream_id) {
    print(`视频流上线: ${stream_id}`);
    
    // 自动开始录制
    start_recording(stream_id, #{
        backend: "nas_storage",
        format: "mp4",
        segment_duration: 300,
    });
    
    // 每 10 秒保存一个关键帧
    schedule_keyframe_capture(stream_id, 10);
    
    // 每 30 秒检测一次危险
    schedule_task(30, || {
        let report = detect_danger(stream_id);
        if report.detected {
            let snapshot = capture_snapshot(stream_id);
            send_alert("安保中心", #{
                type: report.danger_type,
                stream_id: stream_id,
                snapshot_url: snapshot.url,
                confidence: report.confidence,
            });
        }
    });
}

// GB28181 设备上线
fn on_gb28181_device_online(device_id) {
    print(`GB28181 设备上线: ${device_id}`);
    start_gb28181_stream(device_id, #{
        storage: "local_ssd",
        enable_ptz: true,
    });
}
```

---

## 风险评估

### 技术风险

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| GB28181 协议复杂 | 🟡 中 | 分阶段实施，先实现核心功能 |
| PS 流解析难度大 | 🟡 中 | 参考开源实现，充分测试 |
| 大规模并发性能 | 🟡 中 | Worker Pool + 性能测试 |
| 存储成本高 | 🟢 低 | 分层存储 + 关键帧优化 |

### 工程风险

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| 工期延误 | 🟢 低 | 分 3 个里程碑，渐进交付 |
| 设备兼容性 | 🟡 中 | 建立设备测试矩阵 |
| 运维复杂度 | 🟢 低 | 完善文档 + 监控告警 |

---

## 总结

### 核心优势

1. **架构清晰**：Native 插件 + 多存储后端 + 统一抽象
2. **性能优秀**：零拷贝 + Worker Pool + 硬件加速
3. **灵活扩展**：插件化协议 + Rhai 脚本 + 多存储策略
4. **工程可控**：分阶段实施 + 风险可控 + 文档完善

### 下一步行动

**立即开始 Milestone 1 实施**，2 周后交付可用的 RTSP 监控系统，验证架构可行性。

---

**文档版本**: v1.0  
**最后更新**: 2026年02月11日  
**维护者**: FLUX IOT 开发团队
