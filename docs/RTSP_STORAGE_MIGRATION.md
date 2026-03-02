# RTSP 协议迁移到 flux-storage

> 日期: 2026-02-23
> 状态: 🔄 进行中

---

## 📋 迁移目标

将 RTSP 录像从 `FileSystemStorage` 迁移到 `flux-storage + 元数据架构`，实现：
- ✅ 统一的存储管理
- ✅ 元数据索引
- ✅ 录像回放支持
- ✅ 时间范围查询

---

## 🔍 当前实现分析

### 现状

**存储方式**: `FileSystemStorage`（简单文件系统）

**代码位置**:
- `crates/flux-rtspd/src/stream_manager.rs` - RTSP 流管理器
- `crates/flux-rtspd/src/main.rs` - 初始化逻辑

**当前流程**:
```rust
// 1. 初始化 FileSystemStorage
let storage = Arc::new(RwLock::new(FileSystemStorage::new(storage_config)?));

// 2. 创建流管理器
let stream_manager = Arc::new(RtspStreamManager::new(
    storage.clone(),  // ← 使用 FileSystemStorage
    orchestrator.clone(),
    timeshift,
    telemetry.clone(),
));

// 3. 保存 NALU
Self::save_nalu(stream_id, &nalu, storage, orchestrator, timeshift, telemetry)
```

---

## 🎯 迁移方案

### 步骤 1: 修改 RtspStreamManager 结构

**修改前**:
```rust
pub struct RtspStreamManager {
    storage: Arc<RwLock<FileSystemStorage>>,  // ← 旧的
    orchestrator: Arc<SnapshotOrchestrator>,
    streams: Arc<RwLock<HashMap<String, RtspStreamInfo>>>,
    timeshift: Option<Arc<TimeShiftCore>>,
    telemetry: TelemetryClient,
}
```

**修改后**:
```rust
pub struct RtspStreamManager {
    segment_storage: Arc<dyn SegmentStorage>,  // ← 新的（统一接口）
    orchestrator: Arc<SnapshotOrchestrator>,
    streams: Arc<RwLock<HashMap<String, RtspStreamInfo>>>,
    timeshift: Option<Arc<TimeShiftCore>>,
    telemetry: TelemetryClient,
}
```

### 步骤 2: 修改 save_nalu 方法

**修改前**:
```rust
async fn save_nalu(
    stream_id: &StreamId,
    nalu: &H264Nalu,
    storage: &Arc<RwLock<FileSystemStorage>>,
    orchestrator: &Arc<SnapshotOrchestrator>,
    timeshift: &Option<Arc<TimeShiftCore>>,
    telemetry: &TelemetryClient,
) -> Result<()> {
    // 直接写文件
    let mut storage = storage.write().await;
    storage.save_frame(...)?;
}
```

**修改后**:
```rust
async fn save_nalu(
    stream_id: &StreamId,
    nalu: &H264Nalu,
    segment_storage: &Arc<dyn SegmentStorage>,
    orchestrator: &Arc<SnapshotOrchestrator>,
    timeshift: &Option<Arc<TimeShiftCore>>,
    telemetry: &TelemetryClient,
) -> Result<()> {
    // 构造元数据
    let mut metadata = SegmentMetadata::new();
    metadata
        .set("protocol", "rtsp")
        .set("format", "h264")
        .set("timestamp", nalu.timestamp.to_string())
        .set("is_keyframe", nalu.is_keyframe.to_string())
        .set("size", nalu.data.len().to_string());
    
    // 保存到 flux-storage（带元数据）
    segment_storage
        .save_segment_with_metadata(
            stream_id.as_str(),
            nalu.timestamp as u64,
            metadata,
            &nalu.data,
        )
        .await?;
}
```

### 步骤 3: 修改 main.rs 初始化

**修改前**:
```rust
let storage = Arc::new(RwLock::new(FileSystemStorage::new(storage_config)?));

let stream_manager = Arc::new(RtspStreamManager::new(
    storage.clone(),
    orchestrator.clone(),
    timeshift,
    telemetry.clone(),
));
```

**修改后**:
```rust
// 创建 RTSP 专用存储（使用 flux-storage）
use flux_storage::LocalSegmentStorage;
let rtsp_storage = Arc::new(LocalSegmentStorage::with_storage_manager(
    storage_manager.clone(),
    PathBuf::from("./data/rtsp"),
)) as Arc<dyn flux_storage::SegmentStorage>;

let stream_manager = Arc::new(RtspStreamManager::new(
    rtsp_storage,  // ← 使用 flux-storage
    orchestrator.clone(),
    timeshift,
    telemetry.clone(),
));
```

---

## 📊 元数据 Schema

### RTSP 元数据字段

```json
{
  "protocol": "rtsp",
  "format": "h264",
  "timestamp": "1708675200",
  "is_keyframe": "true",
  "size": "102400",
  "codec": "h264",
  "resolution": "1920x1080",
  "fps": "25"
}
```

### 存储路径

```
/data/rtsp/{stream_id}/nalu_{timestamp}.h264
```

---

## 🔄 迁移步骤

### 1. 修改依赖

```toml
# crates/flux-rtspd/Cargo.toml
[dependencies]
flux-storage = { path = "../flux-storage" }
```

### 2. 修改 stream_manager.rs

- [ ] 修改 `RtspStreamManager` 结构
- [ ] 修改构造函数
- [ ] 修改 `save_nalu` 方法
- [ ] 添加元数据记录

### 3. 修改 main.rs

- [ ] 创建 `LocalSegmentStorage`
- [ ] 传递给 `RtspStreamManager`

### 4. 添加录像回放 API

- [ ] 实现 `/rtsp/{stream_id}/playback` API
- [ ] 支持时间范围查询

### 5. 测试验证

- [ ] 推送 RTSP 流
- [ ] 验证元数据保存
- [ ] 测试录像回放

---

## ✅ 预期收益

### 1. 统一管理

- ✅ RTSP 录像和 HLS 分片使用相同的存储系统
- ✅ 统一的元数据索引
- ✅ 统一的查询接口

### 2. 强大查询

```sql
-- 查询某个流的所有录像
SELECT 
    segment_id,
    metadata->>'timestamp' as timestamp,
    metadata->>'is_keyframe' as is_keyframe
FROM storage.segment_metadata
WHERE stream_id = 'rtsp/camera1'
  AND metadata->>'protocol' = 'rtsp'
ORDER BY segment_id DESC;
```

### 3. 录像回放

```bash
# 查询指定时间范围的录像
GET /rtsp/camera1/playback?start_time=2026-02-23T15:00:00Z&duration=600
```

---

## 🎯 实施计划

### 阶段 1: 基础迁移（当前）

- 修改存储接口
- 添加元数据记录
- 保持基本功能

### 阶段 2: 录像回放

- 实现回放 API
- 支持时间范围查询
- 支持关键帧定位

### 阶段 3: 优化

- 缓存优化
- 索引优化
- 性能调优

---

**迁移状态**: 🔄 进行中  
**预计完成**: 2-3 小时  
**风险**: 低（向后兼容）
