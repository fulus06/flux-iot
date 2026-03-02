# HLS 时移回放快速测试指南

> 日期: 2026-02-23
> 状态: ✅ 可执行

---

## 📋 测试准备

### 1. 环境要求

✅ **已检查的依赖**:
- ✅ ffmpeg: `/opt/homebrew/bin/ffmpeg`
- ✅ psql: `/opt/homebrew/opt/libpq/bin/psql`
- ✅ curl: `/usr/bin/curl`

### 2. 需要启动的服务

#### PostgreSQL 数据库

```bash
# 检查 PostgreSQL 是否运行
pg_isready

# 如果未运行，启动 PostgreSQL
brew services start postgresql@15
# 或
pg_ctl -D /opt/homebrew/var/postgresql@15 start

# 创建测试数据库
createdb flux_iot

# 设置环境变量
export DATABASE_URL="postgres://localhost/flux_iot"
```

#### flux-rtmpd 服务

```bash
# 在新终端窗口启动
cd /Volumes/fushilu/workspace/flux-iot
export DATABASE_URL="postgres://localhost/flux_iot"
cargo run -p flux-rtmpd --features postgres
```

---

## 🚀 快速测试步骤

### 方式 1: 自动化测试脚本

```bash
# 确保服务已启动
export DATABASE_URL="postgres://localhost/flux_iot"

# 运行测试脚本
./scripts/test_timeshift.sh
```

**测试脚本会自动**:
1. ✅ 检查依赖
2. ✅ 推送 30 秒测试流
3. ✅ 验证元数据保存
4. ✅ 测试实时播放
5. ✅ 测试时移回放
6. ✅ 测试分片加载
7. ✅ 自动清理

---

### 方式 2: 手动测试（推荐用于调试）

#### 步骤 1: 启动服务（在独立终端）

```bash
# 终端 1: 启动 PostgreSQL（如果未运行）
brew services start postgresql@15

# 终端 2: 启动 flux-rtmpd
cd /Volumes/fushilu/workspace/flux-iot
export DATABASE_URL="postgres://localhost/flux_iot"
cargo run -p flux-rtmpd --features postgres
```

#### 步骤 2: 推送测试流（在新终端）

```bash
# 终端 3: 推送测试流
cd /Volumes/fushilu/workspace/flux-iot

# 如果没有测试视频，生成一个
ffmpeg -f lavfi -i testsrc=duration=60:size=1280x720:rate=25 \
       -f lavfi -i sine=frequency=1000:duration=60 \
       -c:v libx264 -preset ultrafast -c:a aac \
       -y test.mp4

# 推送到 RTMP 服务器
ffmpeg -re -i test.mp4 \
       -c:v libx264 -preset ultrafast -c:a aac \
       -f flv rtmp://localhost:1935/live/test123
```

#### 步骤 3: 验证元数据（在新终端）

```bash
# 终端 4: 查询元数据
export DATABASE_URL="postgres://localhost/flux_iot"

# 等待 10-15 秒让分片生成

# 查询元数据
psql $DATABASE_URL -c "
SELECT 
    segment_id,
    metadata->>'start_time' as start_time,
    metadata->>'duration' as duration,
    metadata->>'size' as size,
    metadata->>'has_keyframe' as has_keyframe
FROM storage.segment_metadata
WHERE stream_id = 'rtmp/live/test123'
  AND metadata->>'protocol' = 'hls'
ORDER BY segment_id DESC
LIMIT 10;
"
```

**预期结果**: 应该看到多条记录

#### 步骤 4: 测试实时播放

```bash
# 获取实时播放列表
curl http://localhost:8082/hls/rtmp/live/test123/index.m3u8

# 应该看到类似输出:
# #EXTM3U
# #EXT-X-VERSION:3
# #EXT-X-TARGETDURATION:10
# #EXTINF:10.000,
# segment_0.ts
# ...
```

#### 步骤 5: 测试时移回放

```bash
# 获取第一个分片的时间
FIRST_TIME=$(psql $DATABASE_URL -t -c "
SELECT metadata->>'start_time'
FROM storage.segment_metadata
WHERE stream_id = 'rtmp/live/test123'
  AND metadata->>'protocol' = 'hls'
ORDER BY segment_id
LIMIT 1;
" | tr -d ' ')

echo "第一个分片时间: $FIRST_TIME"

# 测试基本时移回放
curl "http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=${FIRST_TIME}"

# 测试带时长参数（30秒）
curl "http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=${FIRST_TIME}&duration=30"

# 测试从关键帧开始
curl "http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=${FIRST_TIME}&from_keyframe=true"
```

**预期结果**: 应该看到 M3U8 播放列表，包含 `#EXT-X-PLAYLIST-TYPE:VOD`

#### 步骤 6: 测试分片加载

```bash
# 加载第一个分片
curl -I http://localhost:8082/hls/rtmp/live/test123/segment_0.ts

# 应该看到 HTTP 200 和 Content-Length
```

---

## 🧪 验证清单

### 功能验证

- [ ] PostgreSQL 数据库运行正常
- [ ] flux-rtmpd 服务启动成功
- [ ] RTMP 流推送成功
- [ ] HLS 分片生成（检查 `./data/rtmp/storage/hls/` 目录）
- [ ] 元数据保存到 PostgreSQL
- [ ] 实时播放列表可访问
- [ ] 时移回放 API 返回正确的 M3U8
- [ ] 分片文件可以下载

### 性能验证

```bash
# 测试元数据查询性能
psql $DATABASE_URL << EOF
\timing on
SELECT COUNT(*)
FROM storage.segment_metadata
WHERE stream_id = 'rtmp/live/test123'
  AND metadata->>'protocol' = 'hls';
EOF
```

**预期**: < 5ms

```bash
# 测试时移回放 API 性能
time curl -s "http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=${FIRST_TIME}" > /dev/null
```

**预期**: < 100ms

---

## 🔍 故障排查

### 问题 1: flux-rtmpd 无法启动

**症状**: `cargo run -p flux-rtmpd` 报错

**解决**:
```bash
# 检查编译错误
cargo build -p flux-rtmpd --features postgres

# 检查端口占用
lsof -i :1935  # RTMP 端口
lsof -i :8082  # HTTP 端口
```

### 问题 2: 无法连接 PostgreSQL

**症状**: `connection refused` 或 `database does not exist`

**解决**:
```bash
# 检查 PostgreSQL 状态
pg_isready

# 创建数据库
createdb flux_iot

# 检查连接
psql $DATABASE_URL -c "SELECT 1;"
```

### 问题 3: 元数据未保存

**症状**: 查询 `storage.segment_metadata` 返回 0 条记录

**解决**:
```bash
# 检查 schema 是否存在
psql $DATABASE_URL -c "\dn"

# 手动运行迁移
psql $DATABASE_URL < crates/flux-storage/migrations/001_create_storage_schema.sql

# 检查表是否存在
psql $DATABASE_URL -c "\dt storage.*"
```

### 问题 4: FFmpeg 推流失败

**症状**: `Connection refused` 或 `RTMP handshake failed`

**解决**:
```bash
# 检查 flux-rtmpd 是否监听 1935 端口
lsof -i :1935

# 检查 flux-rtmpd 日志
# 应该看到 "RTMP server listening on 0.0.0.0:1935"
```

### 问题 5: 时移回放返回 404

**症状**: `curl` 返回 404 Not Found

**解决**:
```bash
# 检查路由是否正确
curl http://localhost:8082/health

# 检查元数据是否存在
psql $DATABASE_URL -c "SELECT COUNT(*) FROM storage.segment_metadata WHERE stream_id = 'rtmp/live/test123';"

# 检查时间格式是否正确（必须是 ISO 8601）
# 正确: 2026-02-23T15:00:00Z
# 错误: 2026-02-23 15:00:00
```

---

## 📊 测试结果示例

### 成功的测试输出

```bash
=== HLS 时移回放功能测试 ===

1. 检查依赖...
✅ 依赖检查通过

2. 启动服务...
✅ 服务已启动

3. 推送测试流（30秒）...
   FFmpeg PID: 12345
   等待 15 秒，让系统生成足够的分片...
✅ 测试流推送中

4. 验证元数据...
   找到 3 个 HLS 分片
   
   最近 5 个分片:
 segment_id |        start_time         | duration |  size
------------+---------------------------+----------+--------
          2 | 2026-02-23T16:30:20Z     | 10.0     | 102400
          1 | 2026-02-23T16:30:10Z     | 10.0     | 102400
          0 | 2026-02-23T16:30:00Z     | 10.0     | 102400

✅ 元数据验证通过

5. 测试实时播放...
   URL: http://localhost:8082/hls/rtmp/live/test123/index.m3u8
✅ 实时播放列表获取成功

6. 测试时移回放...
   URL: http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=2026-02-23T16:30:00Z
✅ 时移回放列表获取成功
✅ 带时长参数的时移回放成功
✅ 从关键帧开始的时移回放成功

7. 测试分片加载...
✅ 分片加载成功 (大小: 102400 bytes)

8. 清理...
✅ 清理完成

=== 测试总结 ===

✅ 测试完成！

验证项:
  ✅ HLS 分片保存
  ✅ 元数据记录
  ✅ 实时播放
  ✅ 时移回放
  ✅ 分片加载

架构验证:
  ✅ 零重复存储
  ✅ 元数据索引
  ✅ 时间范围查询
```

---

## 🎯 下一步

测试通过后：

1. **性能测试**
   - 测试大量历史数据查询
   - 测试并发时移请求
   - 压力测试

2. **长时间运行测试**
   - 24 小时连续推流
   - 监控内存使用
   - 监控磁盘空间

3. **生产部署**
   - 配置优化
   - 监控告警
   - 备份策略

---

**测试准备状态**: ✅ **就绪**

**依赖检查**: ✅ **通过**

**下一步**: 启动 PostgreSQL 和 flux-rtmpd 服务，然后运行测试
