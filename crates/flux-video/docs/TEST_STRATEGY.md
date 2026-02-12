# FLUX Video 测试策略

## 测试方案对比

我们实现了两种测试方案，适用于不同的开发阶段：

---

## 方案 1: 模拟数据测试（自动化）✅

### 适用场景
- ✅ 日常开发和调试
- ✅ CI/CD 自动化测试
- ✅ 快速验证功能
- ✅ 性能基准测试

### 特点
- **速度快**: < 2 秒完成所有测试
- **可靠**: 100% 可重复
- **自动化**: 无需人工干预
- **覆盖全**: 16 个测试用例

### 使用方法

```bash
# 运行所有单元测试
cargo test -p flux-video

# 运行集成测试
cargo test -p flux-video --test integration_test

# 运行完整流水线演示
cargo run --example full_pipeline_demo
```

### 测试覆盖

| 模块 | 测试数量 | 类型 |
|------|---------|------|
| Storage | 7 | 单元 + 集成 |
| H.264 Parser | 5 | 单元 + 集成 |
| Keyframe Extractor | 5 | 单元 + 集成 |
| Video Engine | 2 | 集成 |
| **总计** | **19** | **全自动** |

---

## 方案 2: 人工验证测试（可视化）🎯

### 适用场景
- ✅ 功能验收测试
- ✅ 用户体验验证
- ✅ 演示和展示
- ✅ 问题排查

### 特点
- **直观**: Web 播放器可视化
- **真实**: 模拟真实推流场景
- **交互**: 人工确认功能
- **完整**: 端到端全流程

### 架构流程

```
┌─────────────────┐
│  屏幕捕获推流器  │ (模拟摄像头)
│  生成 H.264 流  │
└────────┬────────┘
         │ RTSP
         ↓
┌─────────────────┐
│  flux-video     │ (核心服务)
│  接收 + 存储    │
│  + 关键帧提取   │
└────────┬────────┘
         │ HTTP API
         ↓
┌─────────────────┐
│  Web 播放器     │ (人工验证)
│  实时统计       │
│  + 日志监控     │
└─────────────────┘
```

### 一键启动

```bash
cd /Volumes/fushilu/workspace/flux-iot/crates/flux-video

# 启动完整测试环境
./start_manual_test.sh

# 浏览器会自动打开
# http://localhost:8080/player.html?stream=screen_capture
```

### 验证清单

在 Web 播放器中验证：

- [ ] 点击"连接流"后状态变为"已连接"
- [ ] 统计数据实时更新（帧数、关键帧、数据量）
- [ ] 日志区域显示关键帧接收记录
- [ ] 点击"截图"功能正常
- [ ] 时长计时器正常运行

---

## 测试文件结构

```
flux-video/
├── tests/
│   ├── integration_test.rs        # 集成测试（自动化）
│   └── integration_test.sh        # API 测试脚本
├── examples/
│   ├── full_pipeline_demo.rs      # 完整流水线演示
│   ├── screen_capture_streamer.rs # 屏幕捕获推流器
│   ├── video_server.rs            # HTTP API 服务器
│   ├── rtsp_example.rs            # RTSP 客户端示例
│   └── mock_rtsp_server.rs        # RTSP 模拟服务器
├── static/
│   └── player.html                # Web 播放器
├── docs/
│   ├── MANUAL_TEST_GUIDE.md       # 人工测试指南
│   └── TEST_STRATEGY.md           # 本文档
└── start_manual_test.sh           # 一键启动脚本
```

---

## 推荐的测试流程

### 开发阶段（每次提交前）

```bash
# 1. 快速验证
cargo test -p flux-video

# 2. 完整演示
cargo run --example full_pipeline_demo

# 3. 如果有 UI 变更，运行人工测试
./start_manual_test.sh
```

### 功能验收（Milestone 完成时）

```bash
# 1. 运行所有自动化测试
cargo test -p flux-video
cargo test -p flux-video --test integration_test

# 2. 运行人工验证测试
./start_manual_test.sh

# 3. 填写测试报告
# 参考 docs/MANUAL_TEST_GUIDE.md 中的模板
```

### CI/CD 流程

```yaml
# .github/workflows/test.yml
- name: Run automated tests
  run: |
    cargo test -p flux-video
    cargo test -p flux-video --test integration_test
    cargo run --example full_pipeline_demo
```

---

## 性能基准

### 自动化测试性能

| 测试类型 | 执行时间 | 测试数量 |
|---------|---------|---------|
| 单元测试 | < 1 秒 | 12 个 |
| 集成测试 | < 2 秒 | 8 个 |
| 完整演示 | < 5 秒 | 1 个 |

### 人工测试性能

| 指标 | 预期值 |
|------|--------|
| 启动时间 | < 10 秒 |
| 帧率 | 30 fps |
| 关键帧频率 | 1/秒 |
| 数据吞吐 | ~300 KB/s |

---

## 测试数据说明

### 模拟 H.264 数据格式

```rust
// SPS (Sequence Parameter Set)
[0x00, 0x00, 0x00, 0x01, 0x67, ...]

// PPS (Picture Parameter Set)
[0x00, 0x00, 0x00, 0x01, 0x68, ...]

// IDR (关键帧)
[0x00, 0x00, 0x00, 0x01, 0x65, ...]

// P-frame (普通帧)
[0x00, 0x00, 0x00, 0x01, 0x41, ...]
```

### 测试场景覆盖

| 场景 | 自动化测试 | 人工测试 |
|------|-----------|---------|
| 单流处理 | ✅ | ✅ |
| 多流并发 | ✅ (10路) | ✅ (3路) |
| 关键帧提取 | ✅ | ✅ |
| 数据存储 | ✅ | ✅ |
| 数据查询 | ✅ | ✅ |
| 过期清理 | ✅ | - |
| 错误处理 | ✅ | - |
| Web UI | - | ✅ |
| 实时监控 | - | ✅ |

---

## 故障排查

### 自动化测试失败

```bash
# 查看详细输出
cargo test -p flux-video -- --nocapture

# 运行特定测试
cargo test -p flux-video test_concurrent_stream_processing

# 清理并重试
cargo clean
cargo test -p flux-video
```

### 人工测试问题

```bash
# 查看服务器日志
tail -f /tmp/flux_video_server.log

# 查看推流器日志
tail -f /tmp/flux_screen_streamer.log

# 检查端口占用
lsof -i :8080
lsof -i :8554

# 重启测试环境
pkill -f video_server
pkill -f screen_capture
./start_manual_test.sh
```

---

## 未来扩展

### 计划中的测试增强

1. **真实摄像头测试**
   - 使用 `scrap` crate 捕获真实屏幕
   - 支持 USB 摄像头输入
   - 性能压力测试（100+ 路）

2. **更多协议支持**
   - RTMP 推流测试
   - WebRTC 测试
   - GB28181 协议测试

3. **高级功能测试**
   - 分布式模式测试
   - 故障恢复测试
   - 性能监控测试

---

## 总结

我们建立了完整的双轨测试体系：

### 自动化测试 🤖
- **快速**: 适合日常开发
- **可靠**: 100% 可重复
- **全面**: 19 个测试用例

### 人工验证 👁️
- **直观**: Web 可视化界面
- **真实**: 模拟真实场景
- **交互**: 人工确认功能

**两种方案互补，确保代码质量和用户体验！** ✅

---

## 快速参考

```bash
# 自动化测试（推荐日常使用）
cargo test -p flux-video && cargo run --example full_pipeline_demo

# 人工验证（推荐功能验收）
./start_manual_test.sh

# 查看测试指南
cat docs/MANUAL_TEST_GUIDE.md
```
