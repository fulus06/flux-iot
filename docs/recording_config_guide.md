# 录像系统配置指南

**更新时间**: 2026-02-19 18:05 UTC+08:00  
**状态**: ✅ **完整配置文档**

---

## 🎯 配置概述

录像系统支持完全可配置，所有参数都可以根据实际需求调整。

### 配置层次

```
全局配置（config/recording.toml）
    ↓
协议配置（config/protocols/*_recording.toml）
    ↓
优先级：协议配置 > 全局配置 > 默认值
```

---

## 📋 完整配置参数

### 1. 基础配置

```toml
[recording]
enabled = true              # 是否启用录像
retention_days = 7          # 保留天数（可配置）
```

**说明**：
- `enabled`: 全局开关，可在协议级别覆盖
- `retention_days`: 录像保留天数，超过后自动删除

---

### 2. 分片配置

```toml
[recording.segment]
strategy = "adaptive"       # 分片策略（可配置）
min_duration = 60           # 最小时长（秒，可配置）
max_duration = 300          # 最大时长（秒，可配置）
target_size_mb = 75         # 目标文件大小（MB，可配置）
```

**分片策略选项**：
- `"fixed"`: 固定时长分片
- `"size"`: 固定大小分片
- `"adaptive"`: 自适应分片（推荐）✅

**自适应策略说明**：
```
根据码率自动调整分片时长，保持文件大小在目标范围内

示例：
- 高码率 (4 Mbps) → 2分钟分片 ≈ 60 MB
- 中码率 (2 Mbps) → 3分钟分片 ≈ 45 MB
- 低码率 (1 Mbps) → 5分钟分片 ≈ 37.5 MB
```

**推荐配置**：
```toml
min_duration = 60           # 1 分钟（精确定位）
max_duration = 300          # 5 分钟（避免文件过多）
target_size_mb = 75         # 75 MB（平衡性能）
```

---

### 3. 压缩配置

```toml
[recording.compression]
realtime = "lz4"                # 实时压缩算法（可配置）
archive = "zstd"                # 归档压缩算法（可配置）
longterm = "brotli"             # 长期压缩算法（可配置）
apply_archive_after_hours = 24  # 归档触发时间（可配置）
apply_longterm_after_days = 7   # 长期触发时间（可配置）
```

**压缩算法选项**：
- `"none"`: 不压缩
- `"lz4"`: LZ4（快速，25% 压缩率）
- `"zstd"`: Zstd（平衡，45% 压缩率）✅
- `"brotli"`: Brotli（高压缩，60% 压缩率）✅
- `"lzma"`: LZMA（极限，70% 压缩率）

**压缩算法对比**：

| 算法 | 压缩率 | 速度 | CPU | 推荐场景 |
|------|--------|------|-----|---------|
| none | 0% | 最快 | 无 | 不压缩 |
| lz4 | 25% | 500 MB/s | 低 | 实时录像 ✅ |
| zstd | 45% | 400 MB/s | 中 | 归档存储 ✅ |
| brotli | 60% | 100 MB/s | 高 | 长期归档 ✅ |
| lzma | 70% | 20 MB/s | 很高 | 极限压缩 |

**推荐配置**：
```toml
realtime = "lz4"                # 实时：快速写入
archive = "zstd"                # 归档：平衡性能
longterm = "brotli"             # 长期：最大压缩
apply_archive_after_hours = 24  # 24小时后压缩
apply_longterm_after_days = 7   # 7天后极限压缩
```

---

### 4. 质量配置

```toml
[recording.quality]
realtime = "high"               # 实时质量（可配置）
archive = "medium"              # 归档质量（可配置）
downgrade_after_hours = 24      # 降级触发时间（可配置）
```

**质量选项**：
- `"original"`: 原始质量（不转码，保持原始分辨率和码率）
- `"high"`: 高质量 (1920×1080, 2 Mbps, 25 fps)
- `"medium"`: 中等质量 (1280×720, 1 Mbps, 25 fps)✅
- `"low"`: 低质量 (854×480, 0.5 Mbps, 15 fps)

> 📖 详细参数说明请参考：[录像质量级别详细说明](./recording_quality_levels.md)

**质量对比**：

| 质量 | 分辨率 | 码率 | 文件大小 | 适用场景 |
|------|--------|------|---------|---------|
| original | 原始 | 原始 | 最大 | 重要录像 |
| high | 1080p | 2 Mbps | 900 MB/h | 实时录像 ✅ |
| medium | 720p | 1 Mbps | 450 MB/h | 归档存储 ✅ |
| low | 480p | 0.5 Mbps | 225 MB/h | 长期归档 |

**推荐配置**：
```toml
realtime = "high"               # 实时：1080p
archive = "medium"              # 归档：720p（节省 50%）
downgrade_after_hours = 24      # 24小时后降级
```

---

### 5. 转换配置

```toml
[recording.conversion]
enabled = true                  # 是否启用自动转换（可配置）
trigger_after_hours = 24        # 触发时间（可配置）
target_quality = "medium"       # 目标质量（可配置）
merge_files = true              # 是否合并文件（可配置）
merge_duration = 600            # 合并时长（可配置）
concurrency = 4                 # 并发数（可配置）
```

**转换流程**：
```
定时检查（每小时）
    ↓
查找超过触发时间的文件
    ↓
转码降级（1080p → 720p）
    ↓
重新压缩（LZ4 → Zstd）
    ↓
合并小文件（1分钟 → 10分钟）
    ↓
移动到归档存储
    ↓
删除原始文件
```

**推荐配置**：
```toml
enabled = true                  # 启用自动转换
trigger_after_hours = 24        # 24小时后转换
target_quality = "medium"       # 降级到 720p
merge_files = true              # 合并小文件
merge_duration = 600            # 合并成 10 分钟
concurrency = 4                 # 4 个并发任务
```

---

### 6. 索引配置

```toml
[recording.index]
engine = "sqlite"               # 索引引擎（可配置）
db_path = "./data/recordings.db"
```

**索引引擎选项**：
- `"json"`: JSON 文件（简单，性能差）
- `"sqlite"`: SQLite 数据库（推荐）✅
- `"binary"`: 二进制索引（极致性能）

**索引引擎对比**：

| 引擎 | 查询速度 | 并发性 | 复杂度 | 推荐场景 |
|------|---------|--------|--------|---------|
| json | 5-10 ms | 差 | 简单 | 小规模 |
| sqlite | < 0.5 ms | 优秀 | 中等 | **通用推荐** ✅ |
| binary | < 0.1 ms | 极好 | 复杂 | 超大规模 |

**推荐配置**：
```toml
engine = "sqlite"               # SQLite（推荐）
db_path = "./data/recordings.db"
```

---

### 7. 存储配置

```toml
[recording.storage]
realtime_path = "/mnt/ssd/recordings/realtime"    # 实时路径（可配置）
archive_path = "/mnt/hdd/recordings/archive"      # 归档路径（可配置）
longterm_path = "/mnt/hdd/recordings/longterm"    # 长期路径（可配置）
```

**存储分层**：
```
实时录像（0-24小时）
  ↓ SSD 存储
  高速读写，快速访问

归档录像（1-7天）
  ↓ HDD 存储
  大容量，标准访问

长期归档（7-30天）
  ↓ HDD 存储
  最大压缩，冷存储
```

---

## 🎯 配置示例

### 示例 1: 高质量长期保留（监控场景）

```toml
[recording]
enabled = true
retention_days = 30             # 保留 30 天

[recording.segment]
strategy = "adaptive"
min_duration = 60
max_duration = 300
target_size_mb = 100            # 更大的分片

[recording.compression]
realtime = "lz4"
archive = "zstd"
longterm = "brotli"
apply_archive_after_hours = 48  # 48小时后压缩
apply_longterm_after_days = 7

[recording.quality]
realtime = "high"               # 1080p
archive = "high"                # 保持 1080p（不降级）
downgrade_after_hours = 720     # 30天后才降级

[recording.conversion]
enabled = true
trigger_after_hours = 48
target_quality = "high"         # 保持高质量
merge_files = true
merge_duration = 600
concurrency = 2                 # 降低并发（减少 CPU）
```

### 示例 2: 低成本短期保留（直播场景）

```toml
[recording]
enabled = true
retention_days = 3              # 只保留 3 天

[recording.segment]
strategy = "adaptive"
min_duration = 120              # 2 分钟（减少文件数）
max_duration = 600              # 10 分钟
target_size_mb = 150

[recording.compression]
realtime = "lz4"
archive = "zstd"
longterm = "brotli"
apply_archive_after_hours = 6   # 6小时后快速压缩
apply_longterm_after_days = 1   # 1天后极限压缩

[recording.quality]
realtime = "medium"             # 720p（节省空间）
archive = "low"                 # 480p（最大节省）
downgrade_after_hours = 6       # 6小时后降级

[recording.conversion]
enabled = true
trigger_after_hours = 6
target_quality = "low"          # 降到 480p
merge_files = true
merge_duration = 1800           # 合并成 30 分钟
concurrency = 8                 # 高并发（快速转换）
```

### 示例 3: 平衡方案（推荐）

```toml
[recording]
enabled = true
retention_days = 7              # 保留 7 天

[recording.segment]
strategy = "adaptive"           # 自适应
min_duration = 60               # 1 分钟
max_duration = 300              # 5 分钟
target_size_mb = 75             # 75 MB

[recording.compression]
realtime = "lz4"                # 快速
archive = "zstd"                # 平衡
longterm = "brotli"             # 高压缩
apply_archive_after_hours = 24
apply_longterm_after_days = 7

[recording.quality]
realtime = "high"               # 1080p
archive = "medium"              # 720p
downgrade_after_hours = 24

[recording.conversion]
enabled = true
trigger_after_hours = 24
target_quality = "medium"
merge_files = true
merge_duration = 600            # 10 分钟
concurrency = 4
```

---

## 📊 配置对存储空间的影响

### 100 路流，不同配置的存储空间对比

| 配置方案 | 实时 | 归档 | 长期 | 总计 | 节省 |
|---------|------|------|------|------|------|
| **无优化** | 2.16 TB | 12.96 TB | - | 15.12 TB | - |
| **推荐配置** | 1.62 TB | 3.24 TB | 2.16 TB | 7.02 TB | 54% |
| **低成本配置** | 0.54 TB | 0.54 TB | 0.27 TB | 1.35 TB | 91% |
| **高质量配置** | 2.16 TB | 12.96 TB | 6.48 TB | 21.6 TB | -43% |

---

## 🎯 总结

**所有参数都可配置**：
- ✅ 分片策略和大小
- ✅ 压缩算法和触发时间
- ✅ 质量级别和降级时间
- ✅ 转换触发和合并策略
- ✅ 索引引擎
- ✅ 存储路径

**配置优先级**：
```
协议配置 > 全局配置 > 默认值
```

**推荐配置**：
- 自适应分片（1-5分钟）
- 分层压缩（LZ4 → Zstd → Brotli）
- 质量降级（1080p → 720p）
- SQLite 索引
- 24小时后自动转换

根据实际需求灵活调整！🚀

---

**文档完成时间**: 2026-02-19 18:05 UTC+08:00  
**状态**: ✅ **完整配置指南**
