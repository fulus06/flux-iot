# FLUX IOT 流媒体架构设计方案

> **版本**: v1.0  
> **日期**: 2026-02-20  
> **作者**: FLUX IOT Team  
> **状态**: 设计完成，待实施

---

## 📋 目录

- [1. 需求分析](#1-需求分析)
- [2. 架构设计](#2-架构设计)
- [3. 核心组件](#3-核心组件)
- [4. 工作模式](#4-工作模式)
- [5. 硬件需求](#5-硬件需求)
- [6. 实施方案](#6-实施方案)
- [7. 配置示例](#7-配置示例)

---

## 1. 需求分析

### 1.1 应用场景

| 场景 | 描述 |
|------|------|
| **IoT 设备** | 物联网设备数据采集和传输 |
| **摄像头监控** | 安防监控、实时监控、录像回放 |
| **其他视频源** | 第三方视频流接入 |

### 1.2 协议需求

**输入协议（多种）**：
- RTMP（推流）
- RTSP（摄像头常用）
- SRT（低延迟传输）
- WebRTC（实时通信）
- HTTP-FLV（推流）

**输出协议（多种）**：
- HLS（Web 播放、录像回放）
- HTTP-FLV（低延迟监控）
- RTMP（转推）
- RTSP（摄像头对接）
- WebRTC（实时通信）

### 1.3 客户端类型

- Web 浏览器
- 移动端 App（iOS/Android）
- 桌面客户端

### 1.4 并发规模

- 最小：10 路
- 典型：50-100 路
- 最大：300 路

### 1.5 转码需求

- **可选转码**：根据实际需求决定是否转码
- **不转码时**：硬件要求低（普通服务器）
- **转码时**：需要 GPU 加速

---

## 2. 架构设计

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────┐
│              输入协议层（Protocol Input）             │
│  RTSP | RTMP | SRT | WebRTC | HTTP-FLV              │
└──────────────────┬──────────────────────────────────┘
                   ↓
┌─────────────────────────────────────────────────────┐
│           统一流管理器（StreamManager）               │
│  - 流注册/注销                                        │
│  - 协议归一化                                         │
│  - 模式选择（直通 vs 转码）                          │
│  - 资源监控                                           │
└──────────────────┬──────────────────────────────────┘
                   ↓
         ┌─────────┴─────────┐
         ↓                   ↓
┌──────────────────┐  ┌──────────────────┐
│  直通模式         │  │  转码模式         │
│  (Passthrough)   │  │  (Transcode)     │
│                  │  │                  │
│  ┌────────────┐ │  │  ┌────────────┐  │
│  │ 解封装     │ │  │  │ 解码       │  │
│  │ Demux      │ │  │  │ Decode     │  │
│  └─────┬──────┘ │  │  └─────┬──────┘  │
│        ↓        │  │        ↓         │
│  ┌────────────┐ │  │  ┌────────────┐  │
│  │ 重新封装   │ │  │  │ 转码       │  │
│  │ Remux      │ │  │  │ (GPU加速)  │  │
│  │ (零拷贝)   │ │  │  └─────┬──────┘  │
│  └─────┬──────┘ │  │        ↓         │
│        │        │  │  ┌────────────┐  │
│        │        │  │  │ 编码       │  │
│        │        │  │  │ Encode     │  │
│        │        │  │  └─────┬──────┘  │
│  CPU: 5%       │  │  CPU: 80%        │
│  内存: 100MB   │  │  内存: 2GB       │
│  GPU: 不需要   │  │  GPU: 需要       │
└────────┬───────┘  └────────┬─────────┘
         │                   │
         └─────────┬─────────┘
                   ↓
┌─────────────────────────────────────────────────────┐
│            存储层（Storage Layer）                    │
│  - 分片存储（已实现 ✅）                              │
│  - 多池管理（已实现 ✅）                              │
│  - 冷热分离                                           │
└──────────────────┬──────────────────────────────────┘
                   ↓
┌─────────────────────────────────────────────────────┐
│           输出协议层（Protocol Output）               │
│  HLS | HTTP-FLV | RTMP | RTSP | WebRTC              │
└─────────────────────────────────────────────────────┘
```

### 2.2 设计原则

1. **灵活性优先**：支持直通和转码两种模式
2. **成本可控**：不转码时硬件成本极低
3. **平滑升级**：可以从直通模式逐步升级到转码模式
4. **按需付费**：只为需要转码的流付出成本

---

## 3. 核心组件

### 3.1 统一流管理器（StreamManager）

**职责**：
- 管理所有输入流（RTSP/RTMP/SRT）
- 协议归一化
- 模式选择（直通 vs 转码）
- 资源监控和负载均衡

**接口设计**：

```rust
pub struct StreamManager {
    /// 活跃流列表
    active_streams: Arc<RwLock<HashMap<StreamId, StreamContext>>>,
    
    /// 转码调度器（可选）
    transcode_scheduler: Option<Arc<TranscodeScheduler>>,
    
    /// 输出管理器
    output_manager: Arc<OutputManager>,
    
    /// 资源监控器
    resource_monitor: Arc<ResourceMonitor>,
}

impl StreamManager {
    /// 注册新流（支持多种协议）
    pub async fn register_stream(
        &self,
        protocol: Protocol,
        source_url: String,
        config: StreamConfig,
    ) -> Result<StreamId>;
    
    /// 请求输出流（自动选择协议）
    pub async fn request_output(
        &self,
        stream_id: &StreamId,
        client_type: ClientType,
        quality: QualityPreference,
    ) -> Result<OutputStream>;
    
    /// 获取流状态
    pub async fn get_stream_status(
        &self,
        stream_id: &StreamId,
    ) -> Result<StreamStatus>;
}
```

### 3.2 流配置（StreamConfig）

```rust
pub struct StreamConfig {
    /// 流 ID
    pub stream_id: String,
    
    /// 输入协议
    pub input_protocol: Protocol,
    
    /// 输入 URL
    pub input_url: String,
    
    /// 工作模式
    pub mode: StreamMode,
    
    /// 输出配置
    pub outputs: Vec<OutputConfig>,
    
    /// 是否启用录像
    pub recording: bool,
    
    /// 优先级
    pub priority: Priority,
}

pub enum StreamMode {
    /// 直通模式（零转码）
    Passthrough {
        /// 是否需要重新封装
        remux: bool,
    },
    
    /// 转码模式
    Transcode {
        /// 目标码率列表
        bitrates: Vec<BitrateConfig>,
        
        /// 硬件加速
        hw_accel: Option<HardwareAccel>,
    },
    
    /// 自动模式（根据需求自动选择）
    Auto {
        /// 触发转码的条件
        trigger: TranscodeTrigger,
    },
}

pub enum TranscodeTrigger {
    /// 协议切换时转码（重要！）
    /// 例如：流默认输出 HLS，当有客户端请求 FLV 时自动转码
    ProtocolSwitch,
    
    /// 检测到不同客户端类型时转码
    ClientVariety,
    
    /// 检测到网络差异时转码
    NetworkVariance,
    
    /// 客户端数量超过阈值时转码
    ClientThreshold(usize),
    
    /// 永不转码
    Never,
}
```

### 3.3 直通模式处理器（PassthroughProcessor）

**关键特性**：
- 零转码，使用 FFmpeg 的 `copy` 模式
- CPU 占用极低（~5%）
- 内存占用极低（~100MB/路）
- 不需要 GPU

**实现**：

```rust
pub struct PassthroughProcessor {
    input: InputStream,
    output_format: OutputFormat,
}

impl PassthroughProcessor {
    pub async fn process(&self) -> Result<()> {
        let mut cmd = Command::new("ffmpeg");
        
        cmd.args(&[
            "-i", &self.input.url,
            
            // 关键：使用 copy 编解码器（零转码）
            "-c:v", "copy",  // 视频直接拷贝
            "-c:a", "copy",  // 音频直接拷贝
            
            // 输出格式
            "-f", match self.output_format {
                OutputFormat::HLS => "hls",
                OutputFormat::FLV => "flv",
            },
            
            // HLS 参数
            "-hls_time", "6",
            "-hls_list_size", "10",
            "-hls_flags", "delete_segments",
            
            &self.output_path,
        ]);
        
        cmd.spawn()?;
        Ok(())
    }
}
```

### 3.4 转码模式处理器（TranscodeProcessor）

**关键特性**：
- 多码率转码
- GPU 硬件加速
- CPU 占用高（~80%）
- 内存占用高（~2GB/路）

**实现**：

```rust
pub struct TranscodeProcessor {
    input: InputStream,
    outputs: Vec<TranscodeOutput>,
    hw_accel: Option<HardwareAccel>,
}

impl TranscodeProcessor {
    pub async fn process(&self) -> Result<()> {
        let mut cmd = Command::new("ffmpeg");
        
        cmd.args(&["-i", &self.input.url]);
        
        // 硬件加速
        if let Some(hw) = &self.hw_accel {
            match hw {
                HardwareAccel::NVENC => {
                    cmd.args(&["-hwaccel", "cuda", "-c:v", "h264_nvenc"]);
                }
                HardwareAccel::QSV => {
                    cmd.args(&["-hwaccel", "qsv", "-c:v", "h264_qsv"]);
                }
                _ => {}
            }
        }
        
        // 多码率输出
        for output in &self.outputs {
            cmd.args(&[
                "-b:v", &format!("{}k", output.bitrate),
                "-s", &format!("{}x{}", output.resolution.0, output.resolution.1),
                &output.path,
            ]);
        }
        
        cmd.spawn()?;
        Ok(())
    }
}
```

### 3.5 输出管理器（OutputManager）

**职责**：根据客户端类型自动选择协议

```rust
pub struct OutputManager {
    hls_manager: Arc<HlsManager>,
    flv_server: Arc<HttpFlvServer>,
}

impl OutputManager {
    pub async fn get_output(
        &self,
        stream_id: &StreamId,
        client_type: ClientType,
        quality: QualityPreference,
    ) -> Result<OutputStream> {
        match (client_type, quality) {
            // Web 浏览器 + 自动质量 → HLS（支持 ABR）
            (ClientType::WebBrowser, QualityPreference::Auto) => {
                self.get_hls_stream(stream_id, true).await
            }
            
            // 移动端 + 自动质量 → HLS（省流量）
            (ClientType::Mobile, QualityPreference::Auto) => {
                self.get_hls_stream(stream_id, true).await
            }
            
            // 任意客户端 + 低延迟 → HTTP-FLV
            (_, QualityPreference::LowLatency) => {
                self.get_flv_stream(stream_id).await
            }
            
            // 默认 → HLS
            _ => self.get_hls_stream(stream_id, false).await
        }
    }
}
```

---

## 4. 工作模式

### 4.1 模式对比

| 维度 | 直通模式 | 转码模式 |
|------|---------|---------|
| **CPU 占用** | ~5% | ~80% |
| **内存占用** | ~100MB/路 | ~2GB/路 |
| **GPU 需求** | 不需要 | 需要 |
| **延迟** | 极低 | 中等 |
| **多码率** | 不支持 | 支持 |
| **适用场景** | 内网监控 | 互联网分发 |

### 4.2 模式选择策略

**场景 A：内网监控（推荐直通模式）**
- 所有摄像头编码格式统一（H.264）
- 客户端统一（都是 Web 或都是移动端）
- 网络环境稳定

**场景 B：混合场景（推荐自动模式）**
- 大部分是内网监控
- 少部分需要互联网分发
- 客户端类型多样

**场景 C：互联网分发（推荐转码模式）**
- 多终端访问（Web + 移动端）
- 网络环境复杂
- 需要多码率适配

---

## 5. 硬件需求

### 5.1 硬件需求对比

| 场景 | 模式 | 并发数 | CPU | 内存 | GPU | 服务器成本 |
|------|------|--------|-----|------|-----|-----------|
| **内网监控** | 直通 | 100路 | 8核 | 16GB | 不需要 | ¥5,000 |
| **内网监控** | 直通 | 300路 | 16核 | 32GB | 不需要 | ¥10,000 |
| **互联网分发** | 转码 | 100路 | 16核 | 64GB | RTX 4060 x4 | ¥50,000 |
| **互联网分发** | 转码 | 300路 | 32核 | 128GB | RTX 4060 x10 | ¥150,000 |

**成本差异**：直通模式是转码模式的 **1/10**！

### 5.2 推荐配置

#### 配置 A：纯直通模式（300路）

```
服务器配置：
- CPU: Intel Xeon E5-2680 v4 (16核)
- 内存: 32GB DDR4
- 存储: 2TB NVMe SSD
- 网络: 万兆网卡
- GPU: 不需要

成本: ¥10,000
```

#### 配置 B：按需转码（300路，10%转码）

```
服务器配置：
- CPU: Intel Xeon Gold 6226R (16核)
- 内存: 64GB DDR4
- 存储: 4TB NVMe SSD
- 网络: 万兆网卡
- GPU: NVIDIA RTX 4060 x1

成本: ¥20,000
```

#### 配置 C：全转码（300路）

```
服务器集群（10台）：
每台配置：
- CPU: Intel Xeon Gold 6226R (16核)
- 内存: 64GB DDR4
- 存储: 2TB NVMe SSD
- 网络: 万兆网卡
- GPU: NVIDIA RTX 4060 x1

总成本: ¥150,000
```

---

## 6. 实施方案

### 6.1 实施阶段

#### 阶段 1：基础功能（第1周）

**目标**：实现直通模式，支持 300 路并发

**任务列表**：
1. HTTP-FLV 路由集成（30分钟）
2. 统一流管理器（2小时）
3. 直通模式处理器（1小时）
4. 输出管理器（30分钟）
5. 测试验证（1小时）

**完成后可支持**：
- 300路并发（直通模式）
- HLS + HTTP-FLV 输出
- 硬件成本：¥10,000

#### 阶段 2：转码支持（第2周，按需）

**目标**：添加转码功能

**任务列表**：
1. 转码模式处理器（1天）
2. 自动模式选择（半天）
3. 硬件加速集成（1天）
4. 转码调度器（1天）
5. 测试验证（半天）

**完成后可支持**：
- 按需转码
- 多码率输出
- GPU 硬件加速

#### 阶段 3：高级功能（第3周，可选）

**目标**：完善功能

**任务列表**：
1. ABR 客户端反馈（1天）
2. 负载均衡（1天）
3. 故障转移（1天）
4. 性能优化（2天）

### 6.2 实施优先级

| 优先级 | 功能 | 工作量 | 价值 |
|--------|------|--------|------|
| **P0** | HTTP-FLV 路由集成 | 30分钟 | 高 |
| **P0** | 统一流管理器 | 2小时 | 高 |
| **P0** | 直通模式处理器 | 1小时 | 高 |
| **P1** | 输出管理器 | 30分钟 | 中 |
| **P2** | 转码模式处理器 | 1天 | 中 |
| **P2** | 自动模式选择 | 半天 | 中 |
| **P3** | ABR 支持 | 1天 | 低 |
| **P3** | 负载均衡 | 1天 | 低 |

---

## 7. 配置示例

### 7.1 方案 A：纯直通模式

**适用场景**：
- 内网监控
- 所有摄像头编码格式统一（H.264）
- 客户端统一

**配置**：

```rust
// 注册流
let config = StreamConfig {
    stream_id: "camera-001".to_string(),
    input_protocol: Protocol::RTSP,
    input_url: "rtsp://192.168.1.100:554/stream".to_string(),
    
    // 使用直通模式
    mode: StreamMode::Passthrough { 
        remux: true 
    },
    
    outputs: vec![
        OutputConfig::HLS,
        OutputConfig::HttpFlv,
    ],
    
    recording: true,
    priority: Priority::Normal,
};

stream_manager.register_stream(config).await?;
```

**硬件需求**（300路）：
- CPU: 16核
- 内存: 32GB
- GPU: 不需要
- 成本: ¥10,000

### 7.2 方案 B：按需转码（推荐）

**适用场景**：
- 大部分是内网监控（直通）
- 少部分需要互联网分发（转码）

**配置**：

```rust
let config = StreamConfig {
    stream_id: "camera-002".to_string(),
    input_protocol: Protocol::RTSP,
    input_url: "rtsp://192.168.1.101:554/stream".to_string(),
    
    // 使用自动模式
    mode: StreamMode::Auto {
        // 客户端数量超过 5 个时启用转码
        trigger: TranscodeTrigger::ClientThreshold(5),
    },
    
    outputs: vec![
        OutputConfig::HLS,
        OutputConfig::HttpFlv,
    ],
    
    recording: true,
    priority: Priority::Normal,
};

stream_manager.register_stream(config).await?;
```

**硬件需求**（300路，10%需要转码）：
- CPU: 16核
- 内存: 64GB
- GPU: RTX 4060 x1
- 成本: ¥20,000

### 7.3 方案 C：全转码模式

**适用场景**：
- 互联网视频平台
- 多终端、多网络环境

**配置**：

```rust
let config = StreamConfig {
    stream_id: "camera-003".to_string(),
    input_protocol: Protocol::RTSP,
    input_url: "rtsp://192.168.1.102:554/stream".to_string(),
    
    // 使用转码模式
    mode: StreamMode::Transcode {
        bitrates: vec![
            BitrateConfig {
                name: "high".to_string(),
                bitrate: 2000,
                resolution: (1920, 1080),
                framerate: 25.0,
            },
            BitrateConfig {
                name: "medium".to_string(),
                bitrate: 1000,
                resolution: (1280, 720),
                framerate: 25.0,
            },
            BitrateConfig {
                name: "low".to_string(),
                bitrate: 500,
                resolution: (640, 360),
                framerate: 25.0,
            },
        ],
        hw_accel: Some(HardwareAccel::NVENC),
    },
    
    outputs: vec![
        OutputConfig::HLS,
        OutputConfig::HttpFlv,
    ],
    
    recording: true,
    priority: Priority::High,
};

stream_manager.register_stream(config).await?;
```

**硬件需求**（300路）：
- CPU: 32核
- 内存: 128GB
- GPU: RTX 4060 x10
- 成本: ¥150,000

---

## 8. API 设计

### 8.1 注册流

```http
POST /api/v1/streams
Content-Type: application/json

{
  "stream_id": "camera-001",
  "input_protocol": "rtsp",
  "input_url": "rtsp://192.168.1.100:554/stream",
  "mode": {
    "type": "passthrough",
    "remux": true
  },
  "outputs": ["hls", "flv"],
  "recording": true,
  "priority": "normal"
}
```

### 8.2 获取流输出

```http
GET /api/v1/streams/camera-001/output?client=web&quality=auto

Response:
{
  "stream_id": "camera-001",
  "protocol": "hls",
  "url": "/hls/camera-001/master.m3u8",
  "abr_enabled": true
}
```

### 8.3 获取流状态

```http
GET /api/v1/streams/camera-001/status

Response:
{
  "stream_id": "camera-001",
  "status": "active",
  "mode": "passthrough",
  "clients": 3,
  "bitrate": 2048,
  "cpu_usage": 5.2,
  "memory_usage": 102
}
```

---

## 9. 总结

### 9.1 核心优势

1. **三种方案全支持**：纯直通、按需转码、全转码，三种方案完全支持
2. **灵活切换**：可以在同一系统中混合使用，甚至运行时动态切换
3. **成本可控**：根据实际需求选择方案，成本差异可达 15 倍
4. **平滑升级**：可以从直通模式逐步升级到转码模式
5. **按需付费**：只为需要转码的流付出成本
6. **混合部署**：不同流可以使用不同的模式

### 9.2 关键指标

| 指标 | 直通模式 | 转码模式 |
|------|---------|---------|
| **并发能力** | 300路 | 30路/GPU |
| **CPU 占用** | 5% | 80% |
| **内存占用** | 100MB/路 | 2GB/路 |
| **硬件成本** | ¥10,000 | ¥150,000 |
| **延迟** | < 1秒 | 2-3秒 |

### 9.3 方案选择

**系统支持三种方案灵活切换，根据实际业务需求选择**：

**方案 A：纯直通模式** ✅
- 适用场景：内网监控、统一编码格式
- 硬件成本：¥10,000（300路）
- 优势：成本最低、延迟最低
- 何时使用：所有摄像头格式统一，客户端统一

**方案 B：按需转码** ✅ 推荐
- 适用场景：混合场景（内网+互联网）
- 硬件成本：¥20,000（300路，10%转码）
- 优势：灵活性高、成本可控
- 何时使用：大部分内网监控，少量互联网分发

**方案 C：全转码模式** ✅
- 适用场景：互联网视频平台、多终端适配
- 硬件成本：¥150,000（300路）
- 优势：多码率、最佳用户体验
- 何时使用：多终端、多网络环境、需要 ABR

**重要**：
- ✅ 三种方案可以在同一系统中共存
- ✅ 可以按流配置不同的模式
- ✅ 支持运行时动态切换模式
- ✅ 系统会根据配置自动选择最优处理方式

---

## 10. 混合部署方案

### 10.1 多方案共存

**系统支持在同一部署中混合使用三种方案**：

```rust
// 流 1：内网监控摄像头 → 直通模式
stream_manager.register_stream(StreamConfig {
    stream_id: "camera-internal-001",
    mode: StreamMode::Passthrough { remux: true },
    // ...
}).await?;

// 流 2：重要监控点 → 按需转码
stream_manager.register_stream(StreamConfig {
    stream_id: "camera-important-001",
    mode: StreamMode::Auto {
        trigger: TranscodeTrigger::ClientThreshold(3),
    },
    // ...
}).await?;

// 流 3：互联网直播 → 全转码
stream_manager.register_stream(StreamConfig {
    stream_id: "camera-public-001",
    mode: StreamMode::Transcode {
        bitrates: vec![high, medium, low],
        hw_accel: Some(HardwareAccel::NVENC),
    },
    // ...
}).await?;
```

### 10.2 自动转码触发机制 ⭐

**核心特性：当检测到特定条件时，自动从直通模式切换到转码模式**

#### 触发条件 1：协议切换触发

```rust
// 场景：流默认使用直通模式，当客户端请求不同协议时自动转码

// 初始配置：直通模式
stream_manager.register_stream(StreamConfig {
    stream_id: "camera-001",
    input_protocol: Protocol::RTSP,
    input_url: "rtsp://192.168.1.100:554/stream",
    
    // 自动模式：检测到协议切换时转码
    mode: StreamMode::Auto {
        trigger: TranscodeTrigger::ProtocolSwitch,
    },
}).await?;

// 客户端 1 请求 HLS → 直通模式（重新封装为 HLS）
GET /hls/camera-001/index.m3u8
→ 系统：使用直通模式，FFmpeg -c:v copy

// 客户端 2 请求 HTTP-FLV → 检测到协议不同，自动转码
GET /flv/camera-001.flv
→ 系统：检测到需要同时输出 HLS 和 FLV
→ 自动切换到转码模式
→ 生成多格式输出
```

#### 触发条件 2：多客户端请求触发

```rust
// 场景：单个客户端时直通，多个客户端时转码

stream_manager.register_stream(StreamConfig {
    stream_id: "camera-002",
    mode: StreamMode::Auto {
        // 客户端数量超过 3 个时转码
        trigger: TranscodeTrigger::ClientThreshold(3),
    },
}).await?;

// 1-3 个客户端 → 直通模式
// 4+ 个客户端 → 自动切换到转码模式（生成多码率）
```

#### 触发条件 3：不同客户端类型触发

```rust
// 场景：检测到不同类型客户端时转码

stream_manager.register_stream(StreamConfig {
    stream_id: "camera-003",
    mode: StreamMode::Auto {
        // 检测到客户端类型多样性时转码
        trigger: TranscodeTrigger::ClientVariety,
    },
}).await?;

// 只有 Web 客户端 → 直通模式
// 同时有 Web + 移动端 → 自动转码（生成多码率）
```

#### 触发条件 4：网络质量差异触发

```rust
// 场景：检测到客户端网络质量差异时转码

stream_manager.register_stream(StreamConfig {
    stream_id: "camera-004",
    mode: StreamMode::Auto {
        // 检测到网络质量差异时转码
        trigger: TranscodeTrigger::NetworkVariance,
    },
}).await?;

// 所有客户端网络良好 → 直通模式
// 检测到有客户端网络差 → 自动转码（生成低码率版本）
```

### 10.3 自动转码实现逻辑

```rust
impl StreamManager {
    /// 请求输出流（自动检测是否需要转码）
    pub async fn request_output(
        &self,
        stream_id: &StreamId,
        client_info: ClientInfo,
    ) -> Result<OutputStream> {
        // 1. 获取流上下文
        let context = self.get_stream_context(stream_id).await?;
        
        // 2. 检查当前模式
        match &context.mode {
            StreamMode::Auto { trigger } => {
                // 3. 评估是否需要转码
                let should_transcode = self.evaluate_transcode_need(
                    stream_id,
                    &client_info,
                    trigger
                ).await?;
                
                if should_transcode && !context.is_transcoding {
                    // 4. 自动切换到转码模式
                    info!("Auto-triggering transcode for stream: {}", stream_id);
                    self.switch_to_transcode(stream_id).await?;
                }
            }
            _ => {}
        }
        
        // 5. 返回输出流
        self.get_output_stream(stream_id, &client_info).await
    }
    
    /// 评估是否需要转码
    async fn evaluate_transcode_need(
        &self,
        stream_id: &StreamId,
        client_info: &ClientInfo,
        trigger: &TranscodeTrigger,
    ) -> Result<bool> {
        match trigger {
            // 协议切换触发
            TranscodeTrigger::ProtocolSwitch => {
                let current_protocols = self.get_active_protocols(stream_id).await?;
                let requested_protocol = client_info.preferred_protocol;
                
                // 如果请求的协议与当前不同，触发转码
                Ok(!current_protocols.contains(&requested_protocol))
            }
            
            // 客户端数量触发
            TranscodeTrigger::ClientThreshold(threshold) => {
                let client_count = self.get_client_count(stream_id).await?;
                Ok(client_count >= *threshold)
            }
            
            // 客户端类型多样性触发
            TranscodeTrigger::ClientVariety => {
                let clients = self.get_clients(stream_id).await?;
                let client_types: HashSet<_> = clients
                    .iter()
                    .map(|c| c.client_type)
                    .collect();
                
                // 如果有 2 种以上客户端类型，触发转码
                Ok(client_types.len() > 1)
            }
            
            // 网络质量差异触发
            TranscodeTrigger::NetworkVariance => {
                let clients = self.get_clients(stream_id).await?;
                let bandwidths: Vec<_> = clients
                    .iter()
                    .map(|c| c.bandwidth_estimate)
                    .collect();
                
                if bandwidths.is_empty() {
                    return Ok(false);
                }
                
                let max_bw = bandwidths.iter().max().unwrap();
                let min_bw = bandwidths.iter().min().unwrap();
                
                // 如果带宽差异超过 50%，触发转码
                Ok((max_bw - min_bw) as f64 / *max_bw as f64 > 0.5)
            }
            
            TranscodeTrigger::Never => Ok(false),
        }
    }
    
    /// 切换到转码模式
    async fn switch_to_transcode(&self, stream_id: &StreamId) -> Result<()> {
        info!("Switching stream {} to transcode mode", stream_id);
        
        // 1. 停止当前直通进程
        self.stop_passthrough(stream_id).await?;
        
        // 2. 启动转码进程
        let transcode_config = TranscodeConfig {
            bitrates: vec![
                BitrateConfig::high(),
                BitrateConfig::medium(),
                BitrateConfig::low(),
            ],
            hw_accel: self.detect_hw_accel(),
        };
        
        self.start_transcode(stream_id, transcode_config).await?;
        
        // 3. 更新流状态
        self.update_stream_status(stream_id, StreamStatus::Transcoding).await?;
        
        info!("Stream {} switched to transcode mode successfully", stream_id);
        Ok(())
    }
}
```

### 10.4 动态模式切换

**支持运行时手动切换模式**：

```rust
// 场景：白天使用直通模式，晚上切换到转码模式（更多客户端）

// 切换到转码模式
stream_manager.update_stream_mode(
    "camera-001",
    StreamMode::Transcode {
        bitrates: vec![high, medium, low],
        hw_accel: Some(HardwareAccel::NVENC),
    }
).await?;

// 切换回直通模式
stream_manager.update_stream_mode(
    "camera-001",
    StreamMode::Passthrough { remux: true }
).await?;
```

### 10.5 典型部署案例

#### 案例 1：中小型监控系统（50路）

```
配置：
- 45 路内网监控 → 直通模式
- 5 路重要监控 → 按需转码

硬件：
- CPU: 8核
- 内存: 32GB
- GPU: RTX 4060 x1（仅处理 5 路转码）

成本: ¥12,000
```

#### 案例 2：大型监控平台（300路）

```
配置：
- 200 路内网监控 → 直通模式
- 80 路重要监控 → 按需转码
- 20 路公网直播 → 全转码

硬件：
- CPU: 16核
- 内存: 64GB
- GPU: RTX 4060 x3（处理 100 路转码）

成本: ¥35,000
```

#### 案例 3：互联网视频平台（300路）

```
配置：
- 300 路全部 → 全转码模式

硬件：
- CPU: 32核
- 内存: 128GB
- GPU: RTX 4060 x10

成本: ¥150,000
```

### 10.6 方案选择决策树

```
开始
  ↓
是否需要多码率？
  ├─ 否 → 是否需要重新封装？
  │        ├─ 否 → 方案 A（纯直通）
  │        └─ 是 → 方案 A（直通+重封装）
  │
  └─ 是 → 是否所有流都需要多码率？
           ├─ 否 → 方案 B（按需转码）
           └─ 是 → 方案 C（全转码）
```

### 10.7 成本优化建议

**策略 1：分时段切换**
- 白天（低峰期）：直通模式
- 晚上（高峰期）：转码模式
- 成本节省：~30%

**策略 2：分优先级部署**
- 重要摄像头：转码模式
- 普通摄像头：直通模式
- 成本节省：~50%

**策略 3：渐进式升级**
- 第一阶段：全部直通（¥10,000）
- 第二阶段：部分转码（¥20,000）
- 第三阶段：全部转码（¥150,000）
- 风险降低：可以逐步验证效果

---

## 11. 配置管理

### 11.1 配置位置

**转码触发条件配置在 `flux-config` 包中**：

```
flux-config/
  ├─ src/
  │   ├─ streaming.rs  ← 流媒体配置（新增）
  │   ├─ global.rs     ← 全局配置
  │   ├─ protocol.rs   ← 协议配置
  │   └─ ...
  └─ Cargo.toml
```

### 11.2 配置结构

```rust
// flux-config/src/streaming.rs

/// 流媒体配置
pub struct StreamingConfig {
    /// 转码配置
    pub transcode: TranscodeConfig,
    
    /// 输出协议配置
    pub outputs: Vec<OutputProtocol>,
}

/// 转码配置
pub struct TranscodeConfig {
    /// 是否启用转码
    pub enabled: bool,
    
    /// 工作模式
    pub mode: TranscodeMode,
    
    /// 硬件加速类型
    pub hardware_accel: Option<HardwareAccel>,
    
    /// 目标码率配置
    pub bitrates: Vec<BitrateConfig>,
}

/// 转码模式
pub enum TranscodeMode {
    /// 直通模式
    Passthrough { remux: bool },
    
    /// 转码模式
    Transcode,
    
    /// 自动模式（可配置触发条件）
    Auto { triggers: Vec<TranscodeTrigger> },
}

/// 转码触发条件（可配置）
pub enum TranscodeTrigger {
    /// 协议切换触发
    ProtocolSwitch,
    
    /// 客户端类型多样性触发
    ClientVariety,
    
    /// 网络质量差异触发
    NetworkVariance { threshold: f64 },
    
    /// 客户端数量触发
    ClientThreshold { count: usize },
    
    /// 永不转码
    Never,
}
```

### 11.3 配置文件示例

#### 示例 1：纯直通模式（TOML）

```toml
# config/streaming.toml

[streaming.transcode]
enabled = true
mode = { type = "passthrough", remux = true }

[[streaming.outputs]]
type = "hls"

[[streaming.outputs]]
type = "flv"
```

#### 示例 2：按需转码 - 协议切换触发（推荐）

```toml
[streaming.transcode]
enabled = true

# 自动模式：协议切换时触发转码
mode = { type = "auto", triggers = [
    { type = "protocol_switch" }
]}

# 硬件加速
hardware_accel = "nvenc"

# 目标码率配置
[[streaming.transcode.bitrates]]
name = "high"
bitrate = 2000
resolution = [1920, 1080]
framerate = 25.0
encoder_preset = "fast"

[[streaming.transcode.bitrates]]
name = "medium"
bitrate = 1000
resolution = [1280, 720]
framerate = 25.0
encoder_preset = "fast"

[[streaming.transcode.bitrates]]
name = "low"
bitrate = 500
resolution = [640, 360]
framerate = 25.0
encoder_preset = "veryfast"
```

#### 示例 3：多触发条件组合

```toml
[streaming.transcode]
enabled = true

# 多个触发条件（满足任一即触发）
mode = { type = "auto", triggers = [
    { type = "protocol_switch" },
    { type = "client_threshold", count = 5 },
    { type = "client_variety" },
    { type = "network_variance", threshold = 0.5 }
]}

hardware_accel = "nvenc"

# 使用默认码率配置
```

#### 示例 4：全转码模式

```toml
[streaming.transcode]
enabled = true
mode = { type = "transcode" }
hardware_accel = "nvenc"

# 自定义多码率
[[streaming.transcode.bitrates]]
name = "ultra"
bitrate = 4000
resolution = [1920, 1080]
framerate = 30.0

[[streaming.transcode.bitrates]]
name = "high"
bitrate = 2000
resolution = [1920, 1080]
framerate = 25.0

[[streaming.transcode.bitrates]]
name = "medium"
bitrate = 1000
resolution = [1280, 720]
framerate = 25.0

[[streaming.transcode.bitrates]]
name = "low"
bitrate = 500
resolution = [640, 360]
framerate = 15.0
```

### 11.4 配置加载

```rust
use flux_config::{ConfigLoader, StreamingConfig};

// 加载配置
let config_loader = ConfigLoader::new("config/streaming.toml")?;
let streaming_config: StreamingConfig = config_loader.load()?;

// 使用配置
match streaming_config.transcode.mode {
    TranscodeMode::Passthrough { remux } => {
        println!("使用直通模式，remux: {}", remux);
    }
    TranscodeMode::Auto { triggers } => {
        println!("使用自动模式，触发条件: {:?}", triggers);
    }
    TranscodeMode::Transcode => {
        println!("使用全转码模式");
    }
}
```

### 11.5 运行时修改配置

```rust
// 支持运行时修改触发条件
let mut config = streaming_config.clone();

// 添加新的触发条件
if let TranscodeMode::Auto { ref mut triggers } = config.transcode.mode {
    triggers.push(TranscodeTrigger::ClientThreshold { count: 10 });
}

// 保存配置
config_loader.save(&config)?;
```

### 11.6 配置优先级

```
1. 命令行参数（最高优先级）
2. 环境变量
3. 配置文件（TOML/YAML/JSON）
4. 默认值（最低优先级）
```

### 11.7 配置验证

```rust
impl StreamingConfig {
    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 验证码率配置
        if self.transcode.enabled {
            if self.transcode.bitrates.is_empty() {
                return Err(anyhow!("转码模式下必须配置至少一个码率"));
            }
            
            // 验证码率递增
            let mut prev_bitrate = 0;
            for bitrate in &self.transcode.bitrates {
                if bitrate.bitrate <= prev_bitrate {
                    return Err(anyhow!("码率必须递增"));
                }
                prev_bitrate = bitrate.bitrate;
            }
        }
        
        // 验证输出协议
        if self.outputs.is_empty() {
            return Err(anyhow!("必须配置至少一个输出协议"));
        }
        
        Ok(())
    }
}
```

---

## 12. 附录

### 12.1 相关文档

- [存储架构设计](./storage_architecture_design.md)
- [HLS/FLV 功能实现](./todo.md#51-hlsflv-完善)
- [ABR 控制器设计](../crates/flux-media-core/src/abr/README.md)

### 12.2 参考资料

- [FFmpeg 官方文档](https://ffmpeg.org/documentation.html)
- [HLS 协议规范](https://datatracker.ietf.org/doc/html/rfc8216)
- [NVIDIA NVENC 编程指南](https://developer.nvidia.com/nvidia-video-codec-sdk)

---

**文档结束**
