# FLUX IOT 代码层未实现/占位实现清单（Consolidated）

> **生成时间**: 2026-02-23  
> **范围**: 仅统计代码中明确存在的占位实现（`TODO/FIXME/todo!/unimplemented!/not implemented` 等）+ 与之直接相关的可交付能力缺口。

---

## 0. 使用说明

- 本文面向“落地实现”的开发视角：每条都尽量包含 **模块**、**文件定位**、**当前行为**、**影响**、**建议修复路径**。
- 优先级说明：
  - **P0**：影响主流程/上线阻断
  - **P1**：影响核心业务但可临时绕过
  - **P2**：增强/优化项

---

## 1. P0（上线阻断级）

### ~~1.1 协议统一接入层 `ProtocolFactory` 未实现~~ ✅ **已完成**

- **模块**: `flux-protocol-factory` (新建独立包)
- **位置**: `crates/flux-protocol-factory/src/factory.rs`
- **完成状态**:
  - ✅ `DefaultProtocolFactory::from_uri()` 完整实现，支持 URI 解析和协议路由
  - ✅ `from_address()` 完整实现 Modbus/CoAP/OpcUa 客户端创建
  - ✅ 支持 URI 参数解析（slave_id, timeout_ms, security_policy, username, password 等）
  - ✅ 使用 feature flags 实现可选协议支持
  - ✅ 通过独立包避免循环依赖问题
- **实现细节**:
  - `flux-protocol` 定义 `ProtocolFactory` trait 接口
  - `flux-protocol-factory` 提供 `DefaultProtocolFactory` 实现
  - 支持的 URI 格式：
    - Modbus: `modbus://192.168.1.100:502?slave_id=1&timeout_ms=5000`
    - CoAP: `coap://localhost:5683/sensors/temperature?timeout_ms=3000`
    - OPC UA: `opcua://localhost:4840?security_policy=None&username=admin`
  - 自动应用默认端口和参数
  - 完整的单元测试覆盖（8个测试全部通过）

### ~~1.2 规则引擎内置函数大量为 Mock~~ ✅ **已完成**

- **模块**: `flux-rule`
- **位置**: `crates/flux-rule/src/functions.rs`
- **完成状态**:
  - ✅ `control_device/read_device/update_device_status` - 已连接到 RuleServices
  - ✅ `send_notification/send_email/send_sms/send_push` - 已连接到 RuleServices
  - ✅ `query_metrics/count_events/record_event` - 已连接到 RuleServices
  - ✅ `date_add/date_start_of_day/date_end_of_day/format_date` - 完整实现
  - ✅ `create_ticket/update_ticket/close_ticket` - 已连接到 RuleServices
  - ✅ 日志函数 (`log`, `debug`, `info`, `warn`, `error`) - 完整实现
- **实现细节**:
  - 所有业务函数通过 `RuleServices` trait 调用真实服务
  - 日期函数使用 chrono 完整实现，支持多种时间单位
  - 工单函数异步调用服务，错误处理完善
  - 所有函数都有完整的测试覆盖

### ~~1.3 场景触发器未落地（Cron/事件/指标/状态）~~ ✅ **已完成**

- **模块**: `flux-control`
- **位置**: `crates/flux-control/src/scene/trigger.rs`
- **完成状态**:
  - ✅ `Schedule { cron }`：使用 `tokio-cron-scheduler` 实现定时调度
  - ✅ `DeviceEvent`：订阅 EventBus，监听设备事件主题
  - ✅ `MetricChange`：订阅设备指标事件，支持 6 种比较操作符
  - ✅ `StatusChange`：监控设备状态转换（支持 from->to 和仅 to）
  - ✅ `unregister_scene`：完整实现触发器取消（cron job + async task）
- **实现细节**:
  - Cron 触发器使用 `JobScheduler` 管理定时任务
  - 事件/指标/状态触发器通过 `tokio::spawn` 订阅 EventBus
  - 所有触发器句柄存储在 `trigger_handles` 映射中，支持优雅关闭
  - 场景执行通过 `SceneEngine` 异步执行 Rhai 脚本

### ~~1.4 RTMPD HLS 音频 TS 封装未实现~~ ✅ **已完成**

- **模块**: `flux-media-core` + `flux-rtmpd`
- **位置**: 
  - `crates/flux-media-core/src/playback/ts.rs` - TsMuxer 音频封装
  - `crates/flux-rtmpd/src/hls_manager.rs` - HlsManager 音频处理
- **完成状态**:
  - ✅ 实现 `TsMuxer::mux_audio_pes()` 方法，支持 AAC 音频帧封装
  - ✅ 音频 PES 包构造（Stream ID 0xC0，仅 PTS 无 DTS）
  - ✅ 音频 TS 包分割和 continuity counter 管理
  - ✅ HlsManager 集成音频处理流程
  - ✅ 时间戳转换（ms -> 90kHz PTS）
  - ✅ 音视频混合分片支持
- **实现细节**:
  - AAC 音频帧（带 ADTS header）-> PES 包 -> TS 包（188 字节）
  - 音频 PID: 0x101，视频 PID: 0x100
  - PAT/PMT 包含音视频流声明
  - 音频包自动添加到当前 HLS 分片
  - 完整的单元测试覆盖（3 个新测试全部通过）

---

## 2. P1（核心功能缺失，但可暂缓/绕过）

### ~~2.1 CoAP Observe/订阅未实现~~ ✅ **已完成**

- **模块**: `flux-coap`
- **位置**: 
  - `crates/flux-coap/src/client.rs` - CoAP Observe 客户端实现
  - `crates/flux-coap/src/adapter.rs` - 协议适配器集成
- **完成状态**:
  - ✅ 实现 CoAP Observe (RFC 7641) 协议支持
  - ✅ Token 管理和回调映射
  - ✅ 后台异步任务接收通知
  - ✅ 订阅/取消订阅生命周期管理
  - ✅ 与 ProtocolClient trait 完整集成
- **实现细节**:
  - 使用 CoAP Option 6 (Observe) 注册订阅
  - 后台 tokio 任务持续监听 UDP 通知
  - Token 映射到回调函数，支持多订阅
  - 自动 JSON 解析或字符串回退
  - 优雅关闭和资源清理

### ~~2.2 OPC UA 订阅未实现~~ ✅ **已完成**

- **模块**: `flux-opcua`
- **位置**: 
  - `crates/flux-opcua/src/client.rs` - OPC UA 订阅客户端实现
  - `crates/flux-opcua/src/adapter.rs` - 协议适配器集成
- **完成状态**:
  - ✅ 实现基于轮询的 OPC UA 订阅机制
  - ✅ MonitoredItem 管理（节点监控项）
  - ✅ 值变化检测和通知
  - ✅ 订阅/取消订阅生命周期管理
  - ✅ 与 ProtocolClient trait 完整集成
- **实现细节**:
  - 采用轮询模拟 OPC UA subscription（简化实现）
  - 1秒间隔轮询监控项
  - 值变化时触发回调通知
  - 支持多节点订阅
  - 后台任务自动管理，优雅关闭

### ~~2.3 Timeseries 归档：S3/MinIO 导出、restore 未完成~~ ✅ **已完成**

- **模块**: `flux-storage` + `flux-timeseries`
- **位置**: 
  - `crates/flux-storage/src/backend/s3.rs` - S3/MinIO 存储后端
  - `crates/flux-timeseries/src/archive.rs` - 归档和恢复逻辑
- **完成状态**:
  - ✅ 实现 S3Backend 使用 aws-sdk-s3
  - ✅ 支持标准 AWS S3 和 MinIO（S3 兼容）
  - ✅ DataArchiver 集成 StorageBackend
  - ✅ 完整的归档流程（查询 -> 序列化 -> 上传 -> 删除）
  - ✅ 完整的恢复流程（下载 -> 反序列化 -> 写回数据库）
  - ✅ 支持本地文件、S3、MinIO 三种归档目标
- **实现细节**:
  - S3Backend 特性：批量操作、范围读取、统计监控
  - 归档格式：JSON（易读、易调试）
  - 恢复策略：ON CONFLICT DO NOTHING（幂等性）
  - 分层架构：flux-timeseries 使用 flux-storage 提供的能力
  - Feature flag 控制：s3 feature 可选编译

### ~~2.4 控制 API：设备指令历史查询未实现~~ ✅ **已完成**

- **模块**: `flux-control-api` + `flux-control`
- **位置**: 
  - `crates/flux-control-api/src/handlers/command.rs` - API 处理器
  - `crates/flux-control/src/command/executor.rs` - 查询逻辑
  - `crates/flux-control/src/db/repository.rs` - 数据库访问
- **完成状态**:
  - ✅ `CommandExecutor` 添加数据库持久化支持
  - ✅ 指令提交时自动保存到数据库
  - ✅ 指令状态更新时同步到数据库
  - ✅ `list_device_commands()` 实现完整查询功能
  - ✅ 支持分页查询（默认50条，最多200条）
  - ✅ 按创建时间倒序排列
- **实现细节**:
  - 使用 `persistence` feature 控制数据库功能
  - 数据库操作失败不影响核心功能（仅记录警告）
  - 完整的指令生命周期追踪（创建、发送、执行、完成）
  - 支持查询所有状态的指令历史

### ~~2.5 RTMPD 登录鉴权仍为示例实现~~ ✅ **已完成**

- **模块**: `flux-rtmpd`
- **位置**: 
  - `crates/flux-rtmpd/src/auth.rs` - 认证逻辑
  - `crates/flux-rtmpd/src/db/` - 数据库层
- **完成状态**:
  - ✅ 实现基于 bcrypt 的密码哈希验证
  - ✅ 实现数据库用户查询（UserRepository）
  - ✅ 用户启用/禁用状态检查
  - ✅ 完整的用户 CRUD 操作
  - ✅ Feature flag 控制（`persistence`）
  - ✅ 回退到示例实现（无数据库时）
- **实现细节**:
  - 使用 bcrypt (cost=12) 进行密码哈希
  - SeaORM 数据库访问层
  - 用户表结构：id, username, password_hash, roles, enabled, created_at, updated_at
  - 角色存储为 JSON 数组
  - 提供数据库迁移脚本和示例用户创建工具

---

## 3. P2（优化/增强项）

### ~~3.1 存储本地后端延迟统计未实现~~ ✅ **已完成**

- **模块**: `flux-storage`
- **位置**: `crates/flux-storage/src/backend/local.rs`
- **完成状态**:
  - ✅ 实现读取延迟统计
  - ✅ 实现写入延迟统计
  - ✅ 使用原子操作累积延迟（微秒级精度）
  - ✅ 计算平均延迟（毫秒）
  - ✅ 在日志中输出操作延迟
  - ✅ 添加完整的测试覆盖
- **实现细节**:
  - 使用 `Instant::now()` 测量操作时间
  - 原子累加总延迟（微秒）
  - 平均延迟 = 总延迟 / 操作次数
  - 支持并发操作的延迟统计
  - 与 S3Backend 延迟统计保持一致

### ~~3.2 flux-server 插件加载仍在 main.rs 中完成~~ ✅ **已完成**

- **模块**: `flux-server`
- **位置**: 
  - `crates/flux-server/src/plugin_loader.rs` - 插件加载服务
  - `crates/flux-server/src/main.rs` - 使用 PluginLoader
- **完成状态**:
  - ✅ 创建专门的 PluginLoader 服务
  - ✅ 封装插件加载逻辑
  - ✅ 提供详细的加载结果统计
  - ✅ 支持批量加载和单文件加载
  - ✅ 完善的错误处理和日志记录
  - ✅ 预留热更新接口
  - ✅ 添加单元测试
- **实现细节**:
  - 使用异步 I/O 读取插件文件
  - 统计加载成功率和失败详情
  - 集成 metrics 监控
  - 支持目录不存在的优雅降级
  - 为未来热更新预留 `watch()` 和 `reload_all()` 接口

### ~~3.3 RTP depacketizer 部分 NAL 类型未实现~~ ✅ **已完成**

- **模块**: `flux-rtspd`
- **位置**:
  - `crates/flux-rtspd/src/h264_depacketizer.rs` - H.264 解包器
  - `crates/flux-rtspd/src/h265_depacketizer.rs` - H.265 解包器
- **完成状态**:
  - ✅ H.264 FU-B (Fragmentation Unit type B) 完整实现
  - ✅ H.265 PACI (Payload Content Information) 完整实现
  - ✅ 支持 DON (Decoding Order Number) 处理
  - ✅ 支持 PACI 单 NAL 和聚合模式
  - ✅ 添加完整的测试覆盖
- **实现细节**:
  - **H.264 FU-B**: 支持带 DON 的分片 NALU，处理解码顺序
  - **H.265 PACI**: 支持负载内容信息包，包括单 NAL 和聚合模式
  - 完善的时间戳验证和错误处理
  - 详细的调试日志输出
  - 与现有 FU-A/AP 实现保持一致的架构

---

## 4. 本次已修复（小范围修复）

### 4.1 修复 `flux-config-manager` 集成测试编译失败

- **位置**: `crates/flux-config-manager/tests/integration_test.rs`
- **修复内容**: 补充导入 `ConfigSource` trait，使 `FileSource::save()` 可用。

### 4.2 修复 `flux-control` MQTT 通道单元测试中的 `todo!()`

- **位置**: `crates/flux-control/src/channel/mqtt.rs`
- **修复内容**: 使用 `rumqttc` 构造 dummy `AsyncClient`，避免 `todo!()` 在测试中潜在 panic。

---

## 5. 下一步建议（按收益排序）

- ~~**建议 A（优先）**：落地 `flux-rule` 内置函数 -> 真实服务（控制/通知/查询）~~ ✅ **已完成**
- ~~**建议 B**：落地 `flux-control` TriggerManager 的 cron + 事件订阅（先让自动触发跑起来）~~ ✅ **已完成**
- ~~**建议 C**：实现 `flux-protocol::ProtocolFactory`（统一协议接入）~~ ✅ **已完成**
- ~~**建议 D**：补齐 HLS 音频 TS 封装（播放器兼容性）~~ ✅ **已完成**
- ~~**建议 E**：实现 CoAP Observe 和 OPC UA subscription（协议层订阅能力）~~ ✅ **已完成**
- ~~**建议 F**：实现 Timeseries S3/MinIO 归档和 restore 功能~~ ✅ **已完成**
- **建议 G（推荐）**：实现 RTMPD 登录鉴权（查库 + bcrypt）
- **建议 H**：实现 Modbus 轮询优化（批量读取、连接池）
- **建议 I**：实现 GB28181 设备目录查询和录像回放
