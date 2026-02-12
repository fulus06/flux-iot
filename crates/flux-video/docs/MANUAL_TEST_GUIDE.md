# FLUX Video 人工验证测试指南

## 测试流程：屏幕捕获 → RTSP推流 → flux-video接收 → Web播放器观看

---

## 🎯 测试目标

通过**人工观看 Web 播放器**来验证视频推流是否成功，确保整个流水线正常工作。

---

## 📋 准备工作

### 1. 确保依赖已安装

```bash
cd /Volumes/fushilu/workspace/flux-iot/crates/flux-video
cargo build --examples
```

### 2. 打开 3 个终端窗口

- **终端 1**: 运行屏幕捕获推流器
- **终端 2**: 运行 flux-video 服务器
- **终端 3**: 测试 API 和查看日志

---

## 🚀 测试步骤

### 步骤 1: 启动 flux-video 服务器

**终端 2** 执行：

```bash
cd /Volumes/fushilu/workspace/flux-iot/crates/flux-video
cargo run --example video_server
```

**预期输出**：
```
Video server listening on 0.0.0.0:8080
```

**验证**：在浏览器打开 http://localhost:8080
- 应该看到 "FLUX Video Server" 首页
- 有 3 个链接：Web 播放器、健康检查、查看所有流

---

### 步骤 2: 启动屏幕捕获推流器

**终端 1** 执行：

```bash
cd /Volumes/fushilu/workspace/flux-iot/crates/flux-video
cargo run --example screen_capture_streamer
```

**预期输出**：
```
=== 屏幕捕获推流器 ===

📺 配置:
   分辨率: 1920x1080
   帧率: 30 fps
   时长: 60 秒

📡 推流地址: rtsp://127.0.0.1:8554/screen

💡 使用方法:
   1. 在另一个终端启动 flux-video 服务器
   2. 创建流连接
   3. 在浏览器打开 Web 播放器

🎬 开始捕获屏幕并推流...
```

---

### 步骤 3: 打开 Web 播放器

在浏览器中打开：

```
http://localhost:8080/player.html?stream=screen_capture
```

**预期界面**：
- 🎥 标题：FLUX Video 播放器
- 黑色视频播放区域（显示"等待视频流..."）
- 流信息面板（显示流 ID、推流地址、状态等）
- 控制按钮：连接流、断开连接、截图
- 统计信息：接收帧数、关键帧、数据量、时长
- 日志区域

---

### 步骤 4: 连接视频流

在 Web 播放器页面：

1. **点击 "▶️ 连接流" 按钮**

2. **观察日志区域**，应该看到：
   ```
   [时间] 正在连接到流: screen_capture
   [时间] ✅ 流连接成功！
   [时间] 开始接收视频数据...
   [时间] 🎯 接收关键帧 #1
   [时间] 🎯 接收关键帧 #2
   ...
   ```

3. **观察统计信息**，应该实时更新：
   - 接收帧数：持续增加
   - 关键帧：每秒增加 1 个
   - 数据量：持续增加
   - 时长：计时器运行

4. **观察流信息面板**：
   - 状态：显示 "已连接"（绿色）
   - 推流地址：rtsp://127.0.0.1:8554/screen

---

### 步骤 5: 测试截图功能

在 Web 播放器页面：

1. **点击 "📸 截图" 按钮**

2. **观察日志**，应该看到：
   ```
   [时间] 📸 正在截图...
   [时间] ✅ 截图成功！
   ```

---

### 步骤 6: 验证后端数据

**终端 3** 执行：

```bash
# 查看所有流
curl http://localhost:8080/api/video/streams

# 查看流信息
curl http://localhost:8080/api/video/streams/screen_capture

# 查看健康状态
curl http://localhost:8080/health
```

**预期输出**：
```json
// 所有流
["screen_capture"]

// 流信息
{
  "stream_id": "screen_capture",
  "protocol": "rtsp",
  "url": "rtsp://example.com/stream",
  "state": "connected"
}

// 健康状态
{
  "success": true,
  "message": "Video server is running"
}
```

---

### 步骤 7: 查看生成的文件

```bash
# 查看存储的视频数据
ls -lh demo_data/storage/screen_capture/

# 查看提取的关键帧
ls -lh demo_data/keyframes/screen_capture/
```

**预期**：
- 应该看到按日期/小时组织的目录结构
- 有 .mp4 视频分片文件
- 有 .h264 关键帧文件

---

## ✅ 验收标准

### 必须通过的检查项

- [ ] **服务器启动成功** - 终端 2 显示服务器监听信息
- [ ] **推流器启动成功** - 终端 1 显示推流配置和地址
- [ ] **Web 播放器可访问** - 浏览器能打开播放器页面
- [ ] **流连接成功** - 点击"连接流"后状态变为"已连接"
- [ ] **统计数据更新** - 帧数、关键帧、数据量持续增加
- [ ] **日志正常输出** - 能看到关键帧接收日志
- [ ] **截图功能正常** - 点击截图后显示成功
- [ ] **API 响应正常** - curl 命令返回正确的 JSON
- [ ] **文件正常生成** - demo_data 目录下有视频和关键帧文件

---

## 🎬 完整测试演示

### 自动化测试脚本

创建 `test_manual.sh`：

```bash
#!/bin/bash

echo "=== FLUX Video 人工验证测试 ==="
echo ""

# 1. 启动服务器（后台）
echo "1️⃣  启动 flux-video 服务器..."
cargo run --example video_server > /tmp/video_server.log 2>&1 &
SERVER_PID=$!
sleep 3

# 2. 启动推流器（后台）
echo "2️⃣  启动屏幕捕获推流器..."
cargo run --example screen_capture_streamer > /tmp/streamer.log 2>&1 &
STREAMER_PID=$!
sleep 2

# 3. 测试 API
echo "3️⃣  测试 API..."
echo ""

echo "健康检查:"
curl -s http://localhost:8080/health | jq .
echo ""

echo "创建流:"
curl -s -X POST http://localhost:8080/api/video/streams \
  -H 'Content-Type: application/json' \
  -d '{
    "stream_id": "screen_capture",
    "protocol": "rtsp",
    "url": "rtsp://127.0.0.1:8554/screen"
  }' | jq .
echo ""

sleep 5

echo "查看所有流:"
curl -s http://localhost:8080/api/video/streams | jq .
echo ""

# 4. 提示人工验证
echo "4️⃣  请在浏览器中打开:"
echo "   http://localhost:8080/player.html?stream=screen_capture"
echo ""
echo "5️⃣  点击 '▶️ 连接流' 按钮"
echo ""
echo "6️⃣  观察统计数据是否更新"
echo ""
echo "按 Enter 继续测试，或 Ctrl+C 退出..."
read

# 5. 清理
echo ""
echo "7️⃣  清理测试环境..."
kill $SERVER_PID $STREAMER_PID 2>/dev/null
echo ""
echo "✅ 测试完成！"
```

---

## 📊 预期结果

### 成功的测试应该看到：

1. **Web 播放器界面**
   - 美观的渐变色界面
   - 清晰的流信息显示
   - 实时更新的统计数据
   - 流畅的日志输出

2. **统计数据（运行 30 秒后）**
   - 接收帧数: ~900 帧 (30 fps × 30 秒)
   - 关键帧: ~30 个 (每秒 1 个)
   - 数据量: ~5-10 MB
   - 时长: 00:30

3. **日志输出**
   - 连接成功消息
   - 定期的关键帧接收日志
   - 截图成功消息（如果点击了截图）

4. **文件系统**
   ```
   demo_data/
   ├── storage/
   │   └── screen_capture/
   │       └── 2026-02-11/
   │           └── 18/
   │               ├── 1707651600.mp4
   │               ├── 1707651601.mp4
   │               └── ...
   └── keyframes/
       └── screen_capture/
           ├── screen_capture_1707651600_123.h264
           ├── screen_capture_1707651605_456.h264
           └── ...
   ```

---

## 🐛 常见问题

### 问题 1: 无法连接到服务器

**症状**: 浏览器显示 "无法连接"

**解决**:
```bash
# 检查服务器是否运行
curl http://localhost:8080/health

# 如果失败，重启服务器
cargo run --example video_server
```

### 问题 2: 推流器无法启动

**症状**: 终端 1 显示错误

**解决**:
```bash
# 检查端口是否被占用
lsof -i :8554

# 如果被占用，杀掉进程或更改端口
```

### 问题 3: 统计数据不更新

**症状**: 点击"连接流"后数据不变

**解决**:
1. 检查浏览器控制台是否有错误
2. 确认推流器正在运行
3. 刷新页面重试

### 问题 4: 文件未生成

**症状**: demo_data 目录为空

**解决**:
```bash
# 检查权限
ls -la demo_data/

# 手动创建目录
mkdir -p demo_data/storage demo_data/keyframes
```

---

## 📝 测试报告模板

```markdown
## FLUX Video 人工验证测试报告

**测试日期**: 2026-02-11
**测试人员**: [姓名]
**测试环境**: macOS / Linux / Windows

### 测试结果

- [ ] 服务器启动: ✅ / ❌
- [ ] 推流器启动: ✅ / ❌
- [ ] Web 播放器访问: ✅ / ❌
- [ ] 流连接: ✅ / ❌
- [ ] 统计数据更新: ✅ / ❌
- [ ] 日志输出: ✅ / ❌
- [ ] 截图功能: ✅ / ❌
- [ ] API 响应: ✅ / ❌
- [ ] 文件生成: ✅ / ❌

### 性能数据

- 运行时长: [X] 秒
- 接收帧数: [X] 帧
- 关键帧数: [X] 个
- 数据量: [X] MB
- 平均帧率: [X] fps

### 问题记录

[描述遇到的问题]

### 总体评价

✅ 通过 / ❌ 未通过

### 备注

[其他说明]
```

---

## 🎉 总结

这个测试流程提供了：

1. ✅ **可视化验证** - 通过 Web 播放器直观看到推流效果
2. ✅ **实时监控** - 统计数据和日志实时更新
3. ✅ **人工确认** - 可以手动验证每个功能
4. ✅ **完整流程** - 覆盖从捕获到播放的全链路
5. ✅ **易于调试** - 清晰的日志和错误提示

**这是一个真实、可靠、易用的测试方案！** 🚀
