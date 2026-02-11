# FLUX IOT 飞流物联网平台

<div align="center">

**高性能、可扩展的 Rust 物联网平台**

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-25%2F25%20passing-brightgreen.svg)](docs/test_coverage_report.md)

[功能特性](#-功能特性) • [快速开始](#-快速开始) • [架构设计](#-架构设计) • [文档](#-文档) • [开发指南](#-开发指南)

</div>

---

## 📋 项目简介

FLUX IOT 是一个基于 Rust 构建的现代化物联网平台，专注于高性能、安全性和可扩展性。平台采用插件化架构，支持 Wasm 插件和 Rhai 脚本引擎，为物联网设备管理和数据处理提供灵活的解决方案。

### 核心优势

- 🚀 **高性能**: Rust 零成本抽象，异步 I/O，支持高并发
- 🔒 **内存安全**: 无 GC，无数据竞争，编译期保证安全
- 🔌 **插件化**: Wasm 沙箱插件，热插拔，隔离执行
- 📜 **脚本引擎**: Rhai 动态规则，无需重启即可更新
- 🌐 **协议支持**: MQTT、HTTP/REST API
- 💾 **数据持久化**: SQLite/PostgreSQL，SeaORM
- 📊 **可观测性**: 集成 tracing，多级别日志

---

## ✨ 功能特性

### 1. 消息总线 (EventBus)

- 高性能的发布/订阅模式
- 支持多订阅者广播
- 异步非阻塞处理
- 容量控制和背压处理

### 2. Wasm 插件系统

- **沙箱隔离**: Wasmtime 运行时，安全执行第三方代码
- **多级别日志**: trace/debug/info/warn/error 集成到 Host
- **内存管理**: 自动 alloc/dealloc，防止内存泄漏
- **热重载**: 支持插件动态加载和卸载

**插件应用场景**:
- 协议转换（Modbus、BACnet）
- 数据增强（地理位置、天气）
- 外部服务调用（邮件、Webhook）
- 自定义算法（加密、压缩）

### 3. Rhai 脚本引擎

- 轻量级嵌入式脚本语言
- 动态规则引擎，支持热更新
- 状态持久化（state_get/state_set）
- 访问消息 payload 和 topic

**规则示例**:
```rhai
// 温度告警规则
if payload.temperature > 80.0 {
    print("High temperature alert!");
    return true;
}
```

### 4. MQTT 支持

- 完整的 MQTT 3.1.1 协议支持
- 设备认证和授权
- QoS 0 支持（QoS 1/2 开发中）
- 自动重连和会话恢复

### 5. RESTful API

- 事件发布接口
- 规则管理（CRUD）
- 规则热重载
- 健康检查

### 6. 数据持久化

- SeaORM 多数据库支持
- 自动迁移和表创建
- 事件历史记录
- 设备管理

---

## 🏗️ 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                      FLUX IOT 平台                           │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  HTTP API    │  │ MQTT Broker  │  │   EventBus   │      │
│  │  (Axum)      │  │  (ntex-mqtt) │  │  (broadcast) │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            ↓                                 │
│                   ┌────────────────┐                         │
│                   │  Rule Worker   │                         │
│                   │                │                         │
│                   │  ┌──────────┐  │                         │
│                   │  │ Wasm插件 │  │  ← 预处理/后处理       │
│                   │  └──────────┘  │                         │
│                   │  ┌──────────┐  │                         │
│                   │  │Rhai脚本  │  │  ← 规则判断            │
│                   │  └──────────┘  │                         │
│                   └────────┬───────┘                         │
│                            ↓                                 │
│                   ┌────────────────┐                         │
│                   │ Storage Worker │                         │
│                   │   (SeaORM)     │                         │
│                   └────────────────┘                         │
└─────────────────────────────────────────────────────────────┘
```

### 数据流

```
MQTT 设备消息 → EventBus → Rule Worker
                              ↓
                         Wasm 插件预处理
                              ↓
                         Rhai 规则引擎
                              ↓
                         Wasm 动作插件
                              ↓
                         Storage Worker
```

---

## 🚀 快速开始

### 环境要求

- Rust 1.75+
- SQLite 3.x (或 PostgreSQL)
- Wasm 工具链（用于插件开发）

### 安装

```bash
# 克隆仓库
git clone https://github.com/yourusername/flux-iot.git
cd flux-iot

# 编译项目
cargo build --release

# 编译 Wasm 插件
cargo build --target wasm32-unknown-unknown --release \
  --manifest-path plugins/dummy_plugin/Cargo.toml

# 复制插件到 plugins 目录
cp target/wasm32-unknown-unknown/release/dummy_plugin.wasm plugins/
```

### 配置

创建 `config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 3000

[database]
url = "sqlite://flux.db"

[plugins]
directory = "plugins"
```

### 运行

```bash
# 启动服务器
cargo run -p flux-server

# 或使用 release 版本
./target/release/flux-server
```

### 验证

```bash
# 健康检查
curl http://localhost:3000/health

# 发布事件
curl -X POST http://localhost:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "sensors/temperature",
    "payload": {"device_id": "sensor001", "temperature": 25.5}
  }'

# 创建规则
curl -X POST http://localhost:3000/api/v1/rules \
  -H "Content-Type: application/json" \
  -d '{
    "name": "high_temp_alert",
    "script": "if payload.temperature > 30.0 { return true; }"
  }'
```

---

## 📚 文档

- [API 文档](docs/API.md) - RESTful API 接口说明
- [插件开发指南](docs/PLUGIN_DEV.md) - Wasm 插件开发教程
- [部署指南](docs/DEPLOYMENT.md) - 生产环境部署
- [测试覆盖率报告](docs/test_coverage_report.md) - 单元测试和集成测试
- [插件集成指南](docs/plugin_integration_guide.md) - 插件系统使用
- [系统总结](docs/plugin_system_summary.md) - 架构和设计决策

---

## 🛠️ 开发指南

### 项目结构

```
flux-iot/
├── crates/
│   ├── flux-core/       # 核心模块（EventBus、实体）
│   ├── flux-plugin/     # Wasm 插件管理
│   ├── flux-script/     # Rhai 脚本引擎
│   ├── flux-server/     # HTTP 服务器
│   ├── flux-mqtt/       # MQTT 服务
│   └── flux-types/      # 共享类型定义
├── sdk/
│   └── flux-plugin-sdk/ # Wasm 插件 SDK
├── plugins/
│   └── dummy_plugin/    # 示例插件
├── docs/                # 文档
└── config.toml          # 配置文件
```

### 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定模块测试
cargo test --package flux-core
cargo test --package flux-plugin
cargo test --package flux-server

# 显示测试输出
cargo test --workspace -- --nocapture
```

**测试结果**: 25/25 通过 ✅

### 代码规范

```bash
# 格式化代码
cargo fmt --all

# 运行 Clippy
cargo clippy --workspace -- -D warnings

# 构建文档
cargo doc --no-deps --open
```

### 开发插件

```bash
# 创建新插件
cd plugins
cargo new --lib my_plugin

# 编译插件
cargo build --target wasm32-unknown-unknown --release \
  --manifest-path plugins/my_plugin/Cargo.toml
```

详见 [插件开发指南](docs/PLUGIN_DEV.md)

---

## 🧪 测试

### 测试覆盖率

| 模块 | 测试数量 | 覆盖率 |
|------|---------|--------|
| flux-core | 7 | ~90% |
| flux-plugin | 10 | ~85% |
| flux-script | 2 | ~70% |
| flux-server | 6 | ~75% |
| **总计** | **25** | **~80%** |

详见 [测试覆盖率报告](docs/test_coverage_report.md)

---

## 🗺️ 路线图

### ✅ 已完成

- [x] 核心 EventBus 实现
- [x] Wasm 插件系统
- [x] Rhai 脚本引擎
- [x] MQTT 支持（QoS 0）
- [x] RESTful API
- [x] 数据持久化
- [x] 多级别日志系统
- [x] 单元测试和集成测试

### 🚧 进行中

- [ ] 完善文档
- [ ] 配置管理优化
- [ ] MQTT QoS 1/2 支持

### 📅 计划中

- [ ] Web UI 管理界面
- [ ] 性能优化和基准测试
- [ ] Docker 容器化
- [ ] Kubernetes 部署
- [ ] 监控和告警（Prometheus）
- [ ] 插件市场

---

## 🤝 贡献

欢迎贡献代码、报告问题或提出建议！

### 贡献流程

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

### 代码规范

- 遵循 Rust 官方代码风格
- 运行 `cargo fmt` 和 `cargo clippy`
- 添加必要的测试
- 更新相关文档

---

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

---

## 🙏 致谢

- [Tokio](https://tokio.rs/) - 异步运行时
- [Axum](https://github.com/tokio-rs/axum) - Web 框架
- [Wasmtime](https://wasmtime.dev/) - Wasm 运行时
- [Rhai](https://rhai.rs/) - 嵌入式脚本引擎
- [SeaORM](https://www.sea-ql.org/SeaORM/) - ORM 框架

---

## 📞 联系方式

- 项目主页: https://github.com/yourusername/flux-iot
- 问题反馈: https://github.com/yourusername/flux-iot/issues
- 邮箱: your.email@example.com

---

## 🔧 常用命令

```bash
# 清理端口占用
kill -9 $(lsof -ti:3000)
kill -9 $(lsof -ti:1883)

# 启动服务器
cargo run -p flux-server

# 启动服务器（带日志）
RUST_LOG=debug,wasm_plugin=trace cargo run -p flux-server

# 编译 Wasm 插件
cargo build --target wasm32-unknown-unknown --release

# 运行测试
cargo test --workspace

# 生成文档
cargo doc --no-deps --open
```

---

<div align="center">

**Built with ❤️ using Rust**

[⬆ 回到顶部](#flux-iot-飞流物联网平台)

</div>
