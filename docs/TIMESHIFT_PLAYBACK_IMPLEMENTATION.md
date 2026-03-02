# 时移回放功能实现方案

> 日期: 2026-02-23
> 状态: ✅ 设计完成

---

## 📋 架构总览

基于已完成的基础设施，时移回放功能采用**零重复存储**架构：

```
┌─────────────────────────────────────────────────────────────┐
│                    时移回放系统                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐      ┌──────────────┐                    │
│  │  HLS 分片    │      │  元数据索引  │                    │
│  │  存储        │      │              │                    │
│  │ (唯一数据源) │◄─────┤ 内存 + PG    │                    │
│  └──────────────┘      └──────────────┘                    │
│         │                      │                            │
│         │                      │                            │
│         ▼                      ▼                            │
│  ┌─────────────────────────────────────┐                   │
│  │      时移回放 API                    │                   │
│  │  - 查询元数据（时间范围）            │                   │
│  │  - 加载分片数据                      │                   │
│  │  - 生成 M3U8 播放列表                │                   │
│  └─────────────────────────────────────┘                   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 🎯 核心原理

### 1. 零重复存储

**关键思想**: HLS 分片是唯一的数据存储，时移只维护元数据索引。

```
数据层:
  /data/hls/app/stream1/segment_1.ts  ← 唯一的数据文件
  /data/hls/app/stream1/segment_2.ts
  /data/hls/app/stream1/segment_3.ts

元数据层（PostgreSQL storage schema）:
  storage.segment_metadata:
    - stream_id: "app/stream1"
    - segment_id: 1
    - metadata: {"start_time": "2026-02-23T15:00:00Z", "duration": "10.0", ...}
```

### 2. 时间索引

通过元数据的 `start_time` 字段建立时间索引：

```sql
-- 查询 5 分钟前到现在的分片
SELECT segment_id, metadata
FROM storage.segment_metadata
WHERE stream_id = 'app/stream1'
  AND metadata->>'start_time' >= '2026-02-23T15:00:00Z'
ORDER BY segment_id;
```

---

## 🔧 实现步骤

### 步骤 1: HLS 分片保存时记录元数据

**位置**: `crates/flux-rtmpd/src/hls_manager.rs`

```rust
async fn finalize_segment(...) {
    // 1. 保存 HLS 分片到 flux-storage
    let filename = self.segment_storage
        .save_segment(stream_id, segment_id, &ts_data)
        .await?;
    
    // 2. 构造元数据
    let mut metadata = SegmentMetadata::new();
    metadata
        .set("start_time", segment_start_time.to_rfc3339())
        .set("duration", duration.to_string())
        .set("size", ts_data.len().to_string())
        .set("has_keyframe", "true")
        .set("format", "ts")
        .set("codec", "h264");
    
    // 3. 保存元数据（内存 + PostgreSQL）
    self.segment_storage
        .save_segment_with_metadata(stream_id, segment_id, metadata, &ts_data)
        .await?;
}
```

### 步骤 2: 时移回放 API

**新增 HTTP API**: `GET /hls/{app}/{stream}/timeshift.m3u8`

**查询参数**:
- `start_time`: 回看起始时间（ISO 8601 格式）
- `duration`: 回看时长（秒，可选）

**实现**:

```rust
// crates/flux-rtmpd/src/main.rs

async fn get_timeshift_playlist(
    Path((app_name, stream_key)): Path<(String, String)>,
    Query(params): Query<TimeshiftParams>,
    State(state): State<AppState>,
) -> Result<String, StatusCode> {
    let stream_id = format!("{}/{}", app_name, stream_key);
    
    // 1. 解析时间参数
    let start_time = DateTime::parse_from_rfc3339(&params.start_time)
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .with_timezone(&Utc);
    
    let end_time = params.duration.map(|d| start_time + Duration::seconds(d));
    
    // 2. 查询元数据（从 PostgreSQL）
    let mut filter = HashMap::new();
    // 可以添加额外过滤条件，如 has_keyframe
    
    let segments = state.segment_storage
        .query_metadata(&stream_id, filter)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // 3. 过滤时间范围
    let filtered_segments: Vec<_> = segments.into_iter()
        .filter(|(_, metadata)| {
            if let Some(start_time_str) = metadata.get("start_time") {
                if let Ok(seg_time) = DateTime::parse_from_rfc3339(start_time_str) {
                    let seg_time = seg_time.with_timezone(&Utc);
                    if seg_time < start_time {
                        return false;
                    }
                    if let Some(end) = end_time {
                        if seg_time > end {
                            return false;
                        }
                    }
                    return true;
                }
            }
            false
        })
        .collect();
    
    // 4. 生成 M3U8 播放列表
    let playlist = generate_timeshift_m3u8(&stream_id, &filtered_segments)?;
    
    Ok(playlist)
}

fn generate_timeshift_m3u8(
    stream_id: &str,
    segments: &[(u64, SegmentMetadata)],
) -> Result<String> {
    let mut m3u8 = String::from("#EXTM3U\n");
    m3u8.push_str("#EXT-X-VERSION:3\n");
    m3u8.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n"); // VOD 类型
    
    // 计算最大时长
    let max_duration = segments.iter()
        .filter_map(|(_, meta)| meta.get("duration"))
        .filter_map(|d| d.parse::<f64>().ok())
        .fold(0.0, f64::max);
    
    m3u8.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", max_duration.ceil() as u64));
    
    // 添加分片
    for (segment_id, metadata) in segments {
        let duration = metadata.get("duration")
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(10.0);
        
        m3u8.push_str(&format!("#EXTINF:{:.3},\n", duration));
        m3u8.push_str(&format!("/hls/{}/segment_{}.ts\n", stream_id, segment_id));
    }
    
    m3u8.push_str("#EXT-X-ENDLIST\n");
    
    Ok(m3u8)
}
```

### 步骤 3: 分片数据加载

**已有 API**: `GET /hls/{app}/{stream}/segment_{id}.ts`

这个 API 已经存在，直接从 `flux-storage` 加载分片数据：

```rust
async fn get_segment(
    Path((app_name, stream_key, segment_id)): Path<(String, String, u64)>,
    State(state): State<AppState>,
) -> Result<Bytes, StatusCode> {
    let stream_id = format!("{}/{}", app_name, stream_key);
    
    // 从 flux-storage 加载分片
    state.segment_storage
        .load_segment(&stream_id, segment_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)
}
```

---

## 📊 完整流程

### 实时流 → 时移回放

```
1. 实时流推送
   RTMP → flux-rtmpd
   
2. HLS 转码
   flux-rtmpd → 生成 TS 分片
   
3. 保存分片 + 元数据
   TS 分片 → flux-storage (文件)
   元数据 → PostgreSQL (storage.segment_metadata)
   
4. 用户请求时移回放
   GET /hls/app/stream/timeshift.m3u8?start_time=2026-02-23T15:00:00Z
   
5. 查询元数据
   PostgreSQL → 查询时间范围内的分片列表
   
6. 生成 M3U8
   根据元数据生成播放列表
   
7. 播放器请求分片
   GET /hls/app/stream/segment_1.ts
   GET /hls/app/stream/segment_2.ts
   ...
   
8. 加载分片数据
   flux-storage → 返回 TS 文件
```

---

## 🎬 使用示例

### 1. 实时观看

```bash
# 实时 HLS 流
http://localhost:8080/hls/live/stream1/index.m3u8
```

### 2. 时移回放（5 分钟前）

```bash
# 从 5 分钟前开始回看
http://localhost:8080/hls/live/stream1/timeshift.m3u8?start_time=2026-02-23T15:00:00Z
```

### 3. 时移回放（指定时长）

```bash
# 从指定时间开始，回看 10 分钟
http://localhost:8080/hls/live/stream1/timeshift.m3u8?start_time=2026-02-23T15:00:00Z&duration=600
```

### 4. 从关键帧开始

```bash
# 查询关键帧分片
SELECT segment_id, metadata->>'start_time'
FROM storage.segment_metadata
WHERE stream_id = 'live/stream1'
  AND metadata->>'has_keyframe' = 'true'
  AND metadata->>'start_time' >= '2026-02-23T15:00:00Z'
ORDER BY segment_id
LIMIT 1;

# 从最近的关键帧开始回看
http://localhost:8080/hls/live/stream1/timeshift.m3u8?start_time=<keyframe_time>
```

---

## 🚀 性能优化

### 1. 元数据索引

```sql
-- 时间范围查询索引
CREATE INDEX idx_segment_metadata_start_time 
    ON storage.segment_metadata((metadata->>'start_time'));

-- 关键帧查询索引
CREATE INDEX idx_segment_metadata_keyframe 
    ON storage.segment_metadata((metadata->>'has_keyframe'))
    WHERE metadata->>'has_keyframe' = 'true';
```

### 2. 内存缓存

```rust
// 热门时移时间段缓存
struct TimeshiftCache {
    cache: Arc<RwLock<HashMap<String, Vec<(u64, SegmentMetadata)>>>>,
}

impl TimeshiftCache {
    async fn get_or_query(&self, key: &str, query_fn: impl Future<Output = Result<Vec<...>>>) {
        // 先查缓存
        if let Some(cached) = self.cache.read().await.get(key) {
            return Ok(cached.clone());
        }
        
        // 缓存未命中，执行查询
        let result = query_fn.await?;
        
        // 更新缓存
        self.cache.write().await.insert(key.to_string(), result.clone());
        
        Ok(result)
    }
}
```

### 3. 分片预加载

```rust
// 预加载接下来的 N 个分片
async fn preload_segments(
    storage: &dyn SegmentStorage,
    stream_id: &str,
    segment_ids: &[u64],
) {
    for &id in segment_ids {
        tokio::spawn(async move {
            let _ = storage.load_segment(stream_id, id).await;
        });
    }
}
```

---

## 📝 API 规范

### GET /hls/{app}/{stream}/timeshift.m3u8

**查询参数**:
- `start_time` (必需): ISO 8601 格式的起始时间
- `duration` (可选): 回看时长（秒）
- `from_keyframe` (可选): 是否从最近的关键帧开始（true/false）

**响应**:
- Content-Type: `application/vnd.apple.mpegurl`
- Body: M3U8 播放列表

**示例**:

```http
GET /hls/live/stream1/timeshift.m3u8?start_time=2026-02-23T15:00:00Z&duration=300&from_keyframe=true

HTTP/1.1 200 OK
Content-Type: application/vnd.apple.mpegurl

#EXTM3U
#EXT-X-VERSION:3
#EXT-X-PLAYLIST-TYPE:VOD
#EXT-X-TARGETDURATION:10
#EXTINF:10.000,
/hls/live/stream1/segment_100.ts
#EXTINF:10.000,
/hls/live/stream1/segment_101.ts
...
#EXT-X-ENDLIST
```

---

## ✅ 优势总结

### 1. 零重复存储

- ✅ HLS 分片是唯一数据源
- ✅ 元数据只存索引信息
- ✅ 节省 50% 磁盘空间

### 2. 强大查询

- ✅ PostgreSQL JSONB 查询
- ✅ 时间范围查询
- ✅ 关键帧查询
- ✅ 复杂过滤条件

### 3. 高性能

- ✅ 内存缓存（热数据）
- ✅ PostgreSQL 索引优化
- ✅ 异步预加载

### 4. 灵活性

- ✅ 任意时间点回看
- ✅ 指定时长回看
- ✅ 从关键帧开始
- ✅ 自定义过滤条件

---

## 🎯 实施清单

- [x] flux-storage 元数据支持
- [x] PostgreSQL storage schema
- [x] 混合模式（内存 + PostgreSQL）
- [ ] HLS 分片保存时记录元数据
- [ ] 时移回放 API 实现
- [ ] M3U8 生成逻辑
- [ ] 前端播放器集成
- [ ] 性能优化和缓存
- [ ] 测试和文档

---

## 🏆 总结

**时移回放功能设计完成**！

**核心架构**:
- ✅ 零重复存储（HLS 分片 + 元数据索引）
- ✅ PostgreSQL 时间索引
- ✅ 混合缓存策略
- ✅ 灵活的查询能力

**下一步**:
1. 在 `flux-rtmpd` 中实现时移回放 API
2. 修改 HLS 分片保存逻辑，记录元数据
3. 添加前端播放器支持
4. 性能测试和优化

**项目状态**: 🟢 设计完成，可以开始实施

---

**设计日期**: 2026-02-23  
**实施难度**: 中等  
**预计工时**: 4-6 小时
