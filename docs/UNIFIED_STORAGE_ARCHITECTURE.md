# 统一存储架构设计

> 日期: 2026-02-23
> 状态: ✅ 设计完成

---

## 📋 问题分析

当前项目支持多种流媒体协议，每种协议都有自己的存储需求：

| 协议 | 数据格式 | 当前存储方式 | 是否使用 flux-storage |
|------|---------|-------------|---------------------|
| **HLS** | TS 分片 | flux-storage | ✅ 是 |
| **HTTP-FLV** | FLV 流 | 内存转发 | ❌ 否（实时流） |
| **RTSP** | H.264 NALU | 文件系统 | ❌ 否 |
| **RTMP** | FLV 流 | 内存转发 | ❌ 否（实时流） |

---

## 🎯 统一存储架构

### 核心原则

**所有需要持久化的媒体数据都应该使用 flux-storage + 元数据索引**

```
┌─────────────────────────────────────────────────────────────┐
│                    统一存储层 (flux-storage)                 │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │  HLS 分片    │  │  RTSP 录像   │  │  快照图片    │     │
│  │  (TS 文件)   │  │  (MP4/H264)  │  │  (JPEG/PNG)  │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
│         │                 │                  │              │
│         └─────────────────┴──────────────────┘              │
│                           │                                  │
│                           ▼                                  │
│              ┌─────────────────────────┐                    │
│              │   元数据索引 (PostgreSQL) │                    │
│              │   storage.segment_metadata│                    │
│              └─────────────────────────┘                    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 📊 各协议存储方案

### 1. HLS (HTTP Live Streaming)

**当前状态**: ✅ 已使用 flux-storage

**存储内容**:
- TS 分片文件
- M3U8 播放列表（动态生成）

**元数据**:
```json
{
  "protocol": "hls",
  "format": "ts",
  "start_time": "2026-02-23T15:00:00Z",
  "duration": "10.0",
  "has_keyframe": "true",
  "codec": "h264",
  "resolution": "1920x1080",
  "bitrate": "2000000"
}
```

**存储路径**:
```
/data/hls/{app}/{stream}/segment_{id}.ts
```

---

### 2. HTTP-FLV

**当前状态**: ❌ 仅实时转发，无持久化

**建议**: 
- **实时流**: 不需要持久化（直接从 RTMP 转发）
- **录像**: 如果需要录像，应该转换为 HLS 或 MP4 格式存储

**实现方案**:

```rust
// 可选：FLV 录像功能
async fn record_flv_to_storage(
    storage: &dyn SegmentStorage,
    stream_id: &str,
    flv_data: &[u8],
    timestamp: DateTime<Utc>,
) -> Result<()> {
    // 1. 将 FLV 转换为 MP4 或 TS
    let converted_data = convert_flv_to_mp4(flv_data)?;
    
    // 2. 保存到 flux-storage
    let mut metadata = SegmentMetadata::new();
    metadata
        .set("protocol", "flv")
        .set("format", "mp4")
        .set("start_time", timestamp.to_rfc3339())
        .set("original_format", "flv");
    
    storage.save_segment_with_metadata(
        stream_id,
        timestamp.timestamp() as u64,
        metadata,
        &converted_data,
    ).await?;
    
    Ok(())
}
```

---

### 3. RTSP (Real Time Streaming Protocol)

**当前状态**: ❌ 使用简单文件系统

**存储需求**:
- H.264 NALU 数据
- 关键帧快照
- 录像文件

**建议方案**: 使用 flux-storage

**实现**:

```rust
// crates/flux-rtspd/src/stream_manager.rs

async fn save_nalu(
    stream_id: &StreamId,
    nalu: &H264Nalu,
    storage: &Arc<dyn SegmentStorage>,
) -> Result<()> {
    // 1. 构造元数据
    let mut metadata = SegmentMetadata::new();
    metadata
        .set("protocol", "rtsp")
        .set("format", "h264")
        .set("timestamp", nalu.timestamp.to_string())
        .set("is_keyframe", nalu.is_keyframe.to_string())
        .set("size", nalu.data.len().to_string());
    
    // 2. 保存到 flux-storage
    storage.save_segment_with_metadata(
        stream_id.as_str(),
        nalu.timestamp as u64,
        metadata,
        &nalu.data,
    ).await?;
    
    Ok(())
}
```

**元数据**:
```json
{
  "protocol": "rtsp",
  "format": "h264",
  "timestamp": "1708675200",
  "is_keyframe": "true",
  "size": "102400",
  "codec": "h264",
  "resolution": "1920x1080"
}
```

**存储路径**:
```
/data/rtsp/{stream_id}/nalu_{timestamp}.h264
```

---

### 4. 快照 (Snapshot)

**当前状态**: ❌ 使用独立的快照系统

**建议方案**: 集成到 flux-storage

**实现**:

```rust
async fn save_snapshot(
    storage: &dyn SegmentStorage,
    stream_id: &str,
    image_data: &[u8],
    timestamp: DateTime<Utc>,
) -> Result<()> {
    let mut metadata = SegmentMetadata::new();
    metadata
        .set("type", "snapshot")
        .set("format", "jpeg")
        .set("timestamp", timestamp.to_rfc3339())
        .set("size", image_data.len().to_string())
        .set("width", "1920")
        .set("height", "1080");
    
    storage.save_segment_with_metadata(
        stream_id,
        timestamp.timestamp() as u64,
        metadata,
        image_data,
    ).await?;
    
    Ok(())
}
```

**元数据**:
```json
{
  "type": "snapshot",
  "format": "jpeg",
  "timestamp": "2026-02-23T15:00:00Z",
  "size": "204800",
  "width": "1920",
  "height": "1080"
}
```

---

## 🔄 统一元数据 Schema

### storage.segment_metadata 表设计

```sql
CREATE TABLE storage.segment_metadata (
    id BIGSERIAL PRIMARY KEY,
    stream_id VARCHAR(255) NOT NULL,
    segment_id BIGINT NOT NULL,
    
    -- 通用元数据（JSONB）
    metadata JSONB NOT NULL DEFAULT '{}',
    
    -- 时间戳
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    UNIQUE(stream_id, segment_id)
);

-- 索引
CREATE INDEX idx_metadata_protocol ON storage.segment_metadata((metadata->>'protocol'));
CREATE INDEX idx_metadata_format ON storage.segment_metadata((metadata->>'format'));
CREATE INDEX idx_metadata_timestamp ON storage.segment_metadata((metadata->>'timestamp'));
CREATE INDEX idx_metadata_type ON storage.segment_metadata((metadata->>'type'));
```

### 元数据字段规范

**通用字段**:
- `protocol`: 协议类型（hls, rtsp, flv, snapshot）
- `format`: 数据格式（ts, h264, mp4, jpeg, png）
- `timestamp` 或 `start_time`: 时间戳
- `size`: 数据大小（字节）

**视频特定字段**:
- `duration`: 时长（秒）
- `has_keyframe` 或 `is_keyframe`: 是否关键帧
- `codec`: 编码格式（h264, h265, vp9）
- `resolution`: 分辨率（1920x1080）
- `bitrate`: 比特率

**快照特定字段**:
- `type`: snapshot
- `width`: 宽度
- `height`: 高度

---

## 📁 统一存储路径规范

```
/data/
  ├── hls/
  │   └── {app}/{stream}/segment_{id}.ts
  ├── rtsp/
  │   └── {stream_id}/nalu_{timestamp}.h264
  ├── snapshots/
  │   └── {stream_id}/snapshot_{timestamp}.jpg
  └── recordings/
      └── {stream_id}/record_{timestamp}.mp4
```

---

## 🔍 统一查询接口

### 按协议查询

```rust
// 查询所有 HLS 分片
let mut filter = HashMap::new();
filter.insert("protocol".to_string(), "hls".to_string());
let segments = storage.query_metadata(stream_id, filter).await?;
```

### 按时间范围查询

```sql
-- 查询指定时间范围的所有数据
SELECT segment_id, metadata
FROM storage.segment_metadata
WHERE stream_id = 'live/stream1'
  AND metadata->>'timestamp' >= '2026-02-23T15:00:00Z'
  AND metadata->>'timestamp' < '2026-02-23T16:00:00Z'
ORDER BY segment_id;
```

### 按类型查询

```rust
// 查询所有快照
let mut filter = HashMap::new();
filter.insert("type".to_string(), "snapshot".to_string());
let snapshots = storage.query_metadata(stream_id, filter).await?;

// 查询所有关键帧
let mut filter = HashMap::new();
filter.insert("is_keyframe".to_string(), "true".to_string());
let keyframes = storage.query_metadata(stream_id, filter).await?;
```

---

## 🚀 迁移计划

### 阶段 1: HLS（已完成）

- ✅ HLS 分片使用 flux-storage
- ✅ 元数据索引
- ✅ 时移回放支持

### 阶段 2: RTSP 录像

```rust
// 修改 flux-rtspd
impl RtspStreamManager {
    async fn save_nalu_to_storage(&self, ...) {
        // 使用 flux-storage 保存 NALU
        self.segment_storage
            .save_segment_with_metadata(...)
            .await?;
    }
}
```

### 阶段 3: 快照系统

```rust
// 修改快照保存逻辑
impl SnapshotOrchestrator {
    async fn save_snapshot_to_storage(&self, ...) {
        // 使用 flux-storage 保存快照
        self.segment_storage
            .save_segment_with_metadata(...)
            .await?;
    }
}
```

### 阶段 4: FLV 录像（可选）

```rust
// 如果需要 FLV 录像功能
impl HttpFlvServer {
    async fn record_to_storage(&self, ...) {
        // 转换为 MP4 并保存
    }
}
```

---

## ✅ 优势总结

### 1. 统一管理

- ✅ 所有媒体数据在一个存储系统中
- ✅ 统一的元数据索引
- ✅ 统一的查询接口

### 2. 灵活查询

- ✅ 按协议查询
- ✅ 按时间范围查询
- ✅ 按类型查询
- ✅ 复杂组合查询

### 3. 时移回放

- ✅ HLS 时移回放
- ✅ RTSP 录像回放
- ✅ 快照历史查看
- ✅ 跨协议回放

### 4. 存储优化

- ✅ 多存储池支持
- ✅ 智能存储选择
- ✅ 对象存储支持
- ✅ 统一清理策略

---

## 🎯 实施建议

### 优先级

1. **高优先级**: HLS（已完成）
2. **中优先级**: RTSP 录像、快照系统
3. **低优先级**: FLV 录像（如果需要）

### 实施步骤

1. **RTSP 集成**:
   - 修改 `flux-rtspd` 使用 `flux-storage`
   - 添加元数据记录
   - 实现录像回放 API

2. **快照集成**:
   - 修改快照保存逻辑
   - 使用统一元数据
   - 实现快照历史查询

3. **统一 API**:
   - 创建统一的媒体查询 API
   - 支持跨协议查询
   - 实现统一的回放接口

---

## 🏆 总结

**所有协议都应该使用统一的 flux-storage + 元数据架构**！

**核心架构**:
- ✅ flux-storage: 统一的数据存储
- ✅ PostgreSQL: 统一的元数据索引
- ✅ 通用 key-value 元数据: 灵活的扩展性

**支持的协议**:
- ✅ HLS: 已完成
- 🔄 RTSP: 待迁移
- 🔄 快照: 待迁移
- 🔄 FLV: 可选

**项目状态**: 🟢 设计完成，可以逐步迁移

---

**设计日期**: 2026-02-23  
**实施难度**: 中等  
**预计工时**: 8-12 小时（全部协议）
