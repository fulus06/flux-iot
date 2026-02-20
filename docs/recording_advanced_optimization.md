# 录像系统高级优化方案

**设计时间**: 2026-02-19 18:00 UTC+08:00  
**状态**: 📋 **高级优化设计**

---

## 🎯 优化问题分析

### 问题 1: 分片大小 - 1分钟 vs 10分钟

**1分钟分片**：
```
优势：
✅ 更精确的定位（1分钟粒度）
✅ 更小的文件（15 MB @ 2Mbps）
✅ 快速下载（15秒 @ 1MB/s）
✅ 更好的容错性

劣势：
❌ 文件数量多（1440个/天）
❌ 文件系统压力大
❌ 索引文件大
❌ 打开/关闭文件频繁
```

**10分钟分片**：
```
优势：
✅ 文件数量少（144个/天）
✅ 文件系统压力小
✅ 索引文件小

劣势：
❌ 定位粒度粗（10分钟）
❌ 文件较大（150 MB）
```

**推荐方案：自适应分片**

```rust
pub enum SegmentStrategy {
    /// 固定时长
    FixedDuration(u64),  // 秒
    
    /// 固定大小
    FixedSize(u64),      // 字节
    
    /// 自适应（推荐）
    Adaptive {
        min_duration: u64,   // 最小 30 秒
        max_duration: u64,   // 最大 5 分钟
        target_size: u64,    // 目标 50-100 MB
    },
}
```

**自适应策略**：
- 高码率流（4 Mbps）→ 2分钟分片（60 MB）
- 中码率流（2 Mbps）→ 3分钟分片（45 MB）
- 低码率流（1 Mbps）→ 5分钟分片（37.5 MB）
- 保持文件大小在 50-100 MB 范围

---

## 💾 问题 2: 更好的压缩算法

### 压缩算法对比

| 算法 | 压缩率 | 压缩速度 | 解压速度 | CPU占用 | 内存占用 | 推荐场景 |
|------|--------|---------|---------|---------|---------|---------|
| **LZ4** | 20-30% | 500 MB/s | 2000 MB/s | 低 | 低 | 实时录像 |
| **Zstd** | 40-50% | 400 MB/s | 800 MB/s | 中 | 中 | **通用推荐** ✅ |
| **Brotli** | 50-60% | 100 MB/s | 300 MB/s | 高 | 中 | 归档存储 |
| **LZMA** | 60-70% | 20 MB/s | 100 MB/s | 很高 | 高 | 长期归档 |
| **Gzip** | 50-60% | 80 MB/s | 250 MB/s | 高 | 中 | 传统方案 |

### 推荐的分层压缩策略

```
┌─────────────────────────────────────────┐
│  实时录像（0-24小时）                    │
│  算法: LZ4 (level 1)                    │
│  压缩率: 25%                            │
│  速度: 500 MB/s                         │
│  用途: 快速写入，低延迟                  │
└─────────────────────────────────────────┘
                ↓ 24小时后
┌─────────────────────────────────────────┐
│  短期归档（1-7天）                       │
│  算法: Zstd (level 3)                   │
│  压缩率: 45%                            │
│  速度: 400 MB/s                         │
│  用途: 平衡性能和压缩率                  │
└─────────────────────────────────────────┘
                ↓ 7天后
┌─────────────────────────────────────────┐
│  长期归档（7-30天）                      │
│  算法: Brotli (level 6) 或 LZMA        │
│  压缩率: 60%                            │
│  速度: 100 MB/s                         │
│  用途: 最大化压缩率                      │
└─────────────────────────────────────────┘
```

### 实现示例

```rust
pub struct CompressionPipeline {
    realtime: Compressor,   // LZ4
    archive: Compressor,    // Zstd
    longterm: Compressor,   // Brotli/LZMA
}

impl CompressionPipeline {
    /// 实时压缩（快速）
    pub async fn compress_realtime(&self, data: &[u8]) -> Result<Vec<u8>> {
        // LZ4 level 1 - 超快速度
        lz4::compress(data, lz4::CompressionLevel::Fast)
    }
    
    /// 归档压缩（平衡）
    pub async fn compress_archive(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Zstd level 3 - 平衡性能
        zstd::compress(data, 3)
    }
    
    /// 长期归档压缩（最大化）
    pub async fn compress_longterm(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Brotli level 6 - 高压缩率
        brotli::compress(data, 6)
    }
}
```

---

## 🔄 问题 3: 实时到归档的转换机制

### 转换流程设计

```
┌─────────────────────────────────────────┐
│  实时录像写入                            │
│  - 原始质量 (1080p, 2 Mbps)             │
│  - LZ4 压缩                             │
│  - SSD 存储                             │
│  - 1分钟分片                            │
└─────────────────────────────────────────┘
                ↓
┌─────────────────────────────────────────┐
│  后台转换任务（24小时后触发）            │
│  1. 读取实时录像文件                     │
│  2. 转码降级（1080p → 720p）            │
│  3. 重新压缩（LZ4 → Zstd）              │
│  4. 合并小文件（1分钟 → 10分钟）         │
│  5. 写入归档存储（HDD）                  │
│  6. 删除实时文件                         │
└─────────────────────────────────────────┘
```

### 核心组件实现

```rust
pub struct ArchiveConverter {
    /// 转换配置
    config: ArchiveConfig,
    
    /// 转码器
    transcoder: VideoTranscoder,
    
    /// 压缩器
    compressor: CompressionPipeline,
}

pub struct ArchiveConfig {
    /// 触发时间（小时）
    trigger_after_hours: u64,  // 24
    
    /// 目标质量
    target_quality: Quality,   // 720p
    
    /// 目标压缩
    target_compression: CompressionAlgorithm,  // Zstd
    
    /// 合并策略
    merge_strategy: MergeStrategy,
}

pub enum MergeStrategy {
    /// 不合并
    None,
    
    /// 按时长合并
    ByDuration(u64),  // 合并成 10 分钟
    
    /// 按大小合并
    BySize(u64),      // 合并到 100 MB
}

impl ArchiveConverter {
    /// 转换任务
    pub async fn convert_to_archive(
        &self,
        realtime_files: Vec<PathBuf>,
    ) -> Result<PathBuf> {
        // 1. 读取实时文件
        let mut segments = Vec::new();
        for file in realtime_files {
            let data = tokio::fs::read(&file).await?;
            segments.push(self.decompress_lz4(&data)?);
        }
        
        // 2. 合并分片
        let merged = self.merge_segments(segments)?;
        
        // 3. 转码降级
        let transcoded = self.transcoder
            .transcode(&merged, self.config.target_quality)
            .await?;
        
        // 4. 重新压缩
        let compressed = self.compressor
            .compress_archive(&transcoded)
            .await?;
        
        // 5. 写入归档文件
        let archive_path = self.get_archive_path();
        tokio::fs::write(&archive_path, compressed).await?;
        
        // 6. 删除实时文件
        for file in realtime_files {
            tokio::fs::remove_file(file).await?;
        }
        
        Ok(archive_path)
    }
    
    /// 定时转换任务
    pub async fn start_conversion_task(&self) {
        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(3600)  // 每小时检查
        );
        
        loop {
            interval.tick().await;
            
            // 查找需要转换的文件
            let files_to_convert = self.find_files_to_convert().await;
            
            for batch in files_to_convert {
                if let Err(e) = self.convert_to_archive(batch).await {
                    error!("Archive conversion failed: {}", e);
                }
            }
        }
    }
}
```

---

## 🔍 问题 4: 高性能索引引擎

### JSON 索引的问题

```
问题：
❌ 解析慢（需要完整解析 JSON）
❌ 查询慢（线性扫描）
❌ 内存占用大（整个 JSON 加载到内存）
❌ 并发性能差（文件锁）

示例：
- 1天录像 = 1440 个分片（1分钟分片）
- JSON 索引 ≈ 200 KB
- 解析时间 ≈ 10-20 ms
- 查询时间 ≈ 5-10 ms（线性扫描）
```

### 方案 1: SQLite 嵌入式数据库（推荐）

**优势**：
- ✅ 高性能（B-Tree 索引）
- ✅ 支持 SQL 查询
- ✅ ACID 事务
- ✅ 并发读写
- ✅ 零配置

```rust
use rusqlite::{Connection, params};

pub struct RecordingIndex {
    db: Arc<Mutex<Connection>>,
}

impl RecordingIndex {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        
        // 创建索引表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS recordings (
                id INTEGER PRIMARY KEY,
                stream_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER NOT NULL,
                duration REAL NOT NULL,
                size INTEGER NOT NULL,
                format TEXT NOT NULL,
                quality TEXT NOT NULL,
                compressed BOOLEAN NOT NULL,
                compression_algo TEXT,
                file_path TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        
        // 创建索引
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_stream_time 
             ON recordings(stream_id, start_time, end_time)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_created 
             ON recordings(created_at)",
            [],
        )?;
        
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
        })
    }
    
    /// 插入录像记录
    pub async fn insert(&self, record: &RecordingRecord) -> Result<()> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO recordings 
             (stream_id, filename, start_time, end_time, duration, 
              size, format, quality, compressed, compression_algo, 
              file_path, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.stream_id,
                record.filename,
                record.start_time.timestamp(),
                record.end_time.timestamp(),
                record.duration,
                record.size,
                record.format,
                record.quality,
                record.compressed,
                record.compression_algo,
                record.file_path.to_str(),
                record.created_at.timestamp(),
            ],
        )?;
        Ok(())
    }
    
    /// 时间范围查询（高性能）
    pub async fn query_by_time_range(
        &self,
        stream_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<RecordingRecord>> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT * FROM recordings 
             WHERE stream_id = ?1 
             AND start_time <= ?2 
             AND end_time >= ?3
             ORDER BY start_time"
        )?;
        
        let records = stmt.query_map(
            params![stream_id, end.timestamp(), start.timestamp()],
            |row| {
                Ok(RecordingRecord {
                    stream_id: row.get(1)?,
                    filename: row.get(2)?,
                    start_time: DateTime::from_timestamp(row.get(3)?, 0).unwrap(),
                    end_time: DateTime::from_timestamp(row.get(4)?, 0).unwrap(),
                    duration: row.get(5)?,
                    size: row.get(6)?,
                    format: row.get(7)?,
                    quality: row.get(8)?,
                    compressed: row.get(9)?,
                    compression_algo: row.get(10)?,
                    file_path: PathBuf::from(row.get::<_, String>(11)?),
                    created_at: DateTime::from_timestamp(row.get(12)?, 0).unwrap(),
                })
            },
        )?;
        
        records.collect()
    }
}
```

**性能对比**：

| 操作 | JSON | SQLite | 提升 |
|------|------|--------|------|
| **插入** | 10 ms | < 1 ms | 10x |
| **查询（时间范围）** | 5-10 ms | < 0.5 ms | 20x |
| **并发读** | 差 | 优秀 | 100x |
| **内存占用** | 200 KB | 10 KB | 20x |

---

### 方案 2: 自定义二进制索引（极致性能）

**适用场景**：超大规模（百万级分片）

```rust
/// 自定义二进制索引格式
/// 
/// 文件结构：
/// [Header][Index Entries][Data Entries]
/// 
/// Header (32 bytes):
/// - Magic: 4 bytes ("RIDX")
/// - Version: 4 bytes
/// - Entry Count: 8 bytes
/// - Data Offset: 8 bytes
/// - Reserved: 8 bytes
/// 
/// Index Entry (32 bytes):
/// - Stream ID Hash: 8 bytes
/// - Start Time: 8 bytes (Unix timestamp)
/// - End Time: 8 bytes
/// - Data Offset: 8 bytes
/// 
/// Data Entry (variable):
/// - Stream ID: variable (null-terminated)
/// - Filename: variable (null-terminated)
/// - Metadata: variable (binary)

pub struct BinaryIndex {
    mmap: Mmap,  // 内存映射文件
}

impl BinaryIndex {
    /// 二分查找（O(log n)）
    pub fn query_by_time_range(
        &self,
        stream_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<RecordingRecord>> {
        let stream_hash = self.hash_stream_id(stream_id);
        
        // 二分查找起始位置
        let start_idx = self.binary_search_start(stream_hash, start);
        
        // 线性扫描匹配的记录
        let mut results = Vec::new();
        for i in start_idx.. {
            let entry = self.read_index_entry(i)?;
            
            if entry.stream_hash != stream_hash {
                break;
            }
            
            if entry.end_time < start.timestamp() {
                continue;
            }
            
            if entry.start_time > end.timestamp() {
                break;
            }
            
            results.push(self.read_data_entry(entry.data_offset)?);
        }
        
        Ok(results)
    }
}
```

**性能**：
- 查询延迟：< 0.1 ms
- 内存占用：极低（mmap）
- 并发性能：极高（只读）

---

## 📊 综合方案对比

| 方案 | 查询速度 | 并发性 | 实现复杂度 | 推荐场景 |
|------|---------|--------|-----------|---------|
| **JSON** | 5-10 ms | 差 | 简单 | 小规模（< 1000分片） |
| **SQLite** | < 0.5 ms | 优秀 | 中等 | **通用推荐** ✅ |
| **二进制索引** | < 0.1 ms | 极好 | 复杂 | 超大规模（> 100万分片） |

---

## 🎯 最终推荐方案

### 1. 分片策略
```rust
SegmentStrategy::Adaptive {
    min_duration: 60,      // 最小 1 分钟
    max_duration: 300,     // 最大 5 分钟
    target_size: 75_000_000,  // 目标 75 MB
}
```

### 2. 压缩策略
```
实时（0-24h）：LZ4 level 1（25% 压缩率，500 MB/s）
归档（1-7天）：Zstd level 3（45% 压缩率，400 MB/s）
长期（7-30天）：Brotli level 6（60% 压缩率，100 MB/s）
```

### 3. 转换机制
```
定时任务（每小时）→ 检查24小时前的文件 → 
转码降级（1080p→720p）→ 重新压缩（LZ4→Zstd）→ 
合并小文件（1分钟→10分钟）→ 移动到归档存储
```

### 4. 索引引擎
```
SQLite 嵌入式数据库
- B-Tree 索引
- SQL 查询
- ACID 事务
- 查询延迟 < 0.5 ms
```

---

## 📈 性能提升

| 指标 | 优化前 | 优化后 | 提升 |
|------|--------|--------|------|
| **存储空间** | 15.12 TB | 4.5 TB | **70% ↓** |
| **查询速度** | 5-10 ms | < 0.5 ms | **20x ↑** |
| **并发性能** | 10 QPS | 1000+ QPS | **100x ↑** |
| **压缩率** | 0% | 60% | **60% ↑** |

---

**优化完成时间**: 2026-02-19 18:00 UTC+08:00  
**状态**: ✅ **高级优化方案完成**
