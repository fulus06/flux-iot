# FLUX IOT 待办事项清单

## 🔥 高优先级任务

### 1. 协议层功能完善

#### 1.1 RTSP 协议（完成度 100%）🎉✅
- [x] DESCRIBE/SETUP/PLAY 信令实现
- [x] RTP 接收和解析
- [x] H264 RTP 解包（单包/STAP-A/FU-A）
- [x] H265 RTP 解包（单包/AP/FU）
- [x] AAC 音频解包（RFC 3640）
- [x] RTCP 支持（SR/RR 报文、丢包统计、抖动）
- [x] TCP 传输模式（RTP over TCP Interleaved）
- [x] UDP 多播支持（IGMP 协议集成）
- [x] 拉流管理和会话控制
- [x] Telemetry 集成（stream_start/stop, write_ok/err）
- [x] 完整测试覆盖（17 个单元测试）

> **注意**：TCP 多播在技术上不存在（TCP 是点对点协议），只有 UDP 多播。
> **状态**：RTSP 协议已 100% 完成，生产就绪！

#### 1.2 SRT 协议（完成度 100%）✅
**实施策略**：从头实现完整 SRT 协议  
**预计工期**：4-6 周 | **详细规划**：`docs/srt_protocol_plan.md`

**阶段 1：基础功能（40%）✅**
- [x] SRT 包结构（数据包/控制包）
- [x] SRT 握手协议（4 次握手）
- [x] 握手状态机
- [x] SRT Socket 基础结构
- [x] Listener 模式（服务器监听）
- [x] Caller 模式（客户端连接）
- [x] 连接状态管理
- [x] KeepAlive 机制

**阶段 2：可靠性保障（30%）✅**
- [x] 发送缓冲区（Send Buffer）
- [x] 接收缓冲区（Receive Buffer）
- [x] ACK 机制（确认）
- [x] NAK 机制（丢包通知）
- [x] 自动重传（ARQ）
- [x] 乱序包重组
- [x] 丢包检测
- [x] 22 个单元测试通过

**阶段 3：性能优化（20%）✅**
- [x] RTT 测量和统计（SRTT、RTTVAR、RTO）
- [x] 拥塞控制（AIMD 算法）
  - [x] 慢启动（Slow Start）
  - [x] 拥塞避免（Congestion Avoidance）
  - [x] 快速恢复（Fast Recovery）
- [x] 动态带宽估计
- [x] 流量整形（Token Bucket）
- [x] 29 个单元测试通过

**阶段 4：高级特性（10%）✅**
- [x] 统计信息收集和导出
- [x] JSON 格式统计
- [x] 集成测试（36 个测试通过）
> **状态**：SRT 协议已 100% 完成，生产就绪！  
> **详细报告**：`docs/srt_protocol_complete.md`  
> **实现方式**：从头实现（非第三方库集成）

**核心特性**：
- ✅ 完整的 4 次握手协议
- ✅ ARQ 可靠传输（ACK/NAK）
- ✅ AIMD 拥塞控制
- ✅ 动态带宽估计
- ✅ 流量整形（Token Bucket）
- ✅ 完整的统计信息
- ✅ 36 个测试全部通过

**技术栈**：
- 纯 Rust 实现，无 unsafe 代码
- 基于 Tokio 异步运行时
- ~3100 行代码
- 生产就绪

### 2. 配置管理（完成度 100%）✅

**详细规划**：`docs/config_management_plan.md`

#### 2.1 动态配置热更新
**阶段 1：核心框架（30%）✅**
- [x] 创建 flux-config-manager crate
- [x] ConfigManager 核心实现
- [x] ConfigSource trait 抽象
- [x] FileSource 实现（TOML/JSON）
- [x] 版本管理器
- [x] 配置监听器（文件监听）

**阶段 2：热更新机制（20%）✅**
- [x] 自动重载逻辑
- [x] 变更通知机制
- [x] 订阅者模式
- [x] 错误处理

**阶段 3：数据库支持（30%）✅**
- [x] SqliteSource 实现
- [x] PostgresSource 实现
- [x] 数据库 schema 自动创建
- [x] 版本管理集成

**阶段 4：校验和高级特性（20%）✅**
- [x] ConfigValidator 实现
- [x] RangeRule（范围校验）
- [x] CustomRule（自定义校验）
- [x] 配置回滚功能
- [x] 15+ 个测试通过（单元测试 + 集成测试）

> **状态**：配置管理系统已 100% 完成，生产就绪！  
> **核心特性**：
> - ✅ 多数据源支持（File/SQLite/PostgreSQL）
> - ✅ 热更新无需重启
> - ✅ 版本管理和回滚
> - ✅ 配置校验
> - ✅ 变更通知

> **设计方案**：
> 1. **机制设计**：统一 `ConfigManager`（内部 watch channel + reload），支持 file/sqlite/postgres 三类触发方式；
> 2. **最小落地改造**：不大改你现有业务代码，只把 `AppState.config` 从"固定值"升级为"可读最新快照的句柄"（例如 `watch::Receiver<AppConfig>`），并提供一个 `state.config()` 便捷读取方法。

### 3. 生产环境特性

#### 3.1 监控和告警完善（完成度 100%）✅
**详细规划**：`docs/monitoring_alerting_plan.md`

**阶段 1：指标收集（40%）✅**
- [x] 创建 flux-metrics crate
- [x] Prometheus 指标完善（延迟分位数、吞吐量、资源使用率）
- [x] MetricsCollector 实现（18+ 指标类型）
- [x] SystemMetricsCollector（CPU/内存/磁盘）
- [x] Grafana Dashboard 模板（系统概览）
- [x] Prometheus 告警规则（系统/应用/业务）

**阶段 2：告警引擎（30%）✅**
- [x] 实时告警规则引擎（AlertEngine）
- [x] 阈值规则（ThresholdRule）
- [x] 自定义规则接口（AlertRule trait）
- [x] 告警状态管理（Firing/Resolved）
- [x] 告警历史记录

**阶段 3：通知渠道（20%）✅**
- [x] Webhook 通知器
- [x] 钉钉机器人通知（Markdown 格式）
- [x] 邮件通知器
- [x] 通知管理器（NotificationManager）
- [x] 批量通知支持

**阶段 4：聚合降噪（10%）✅**
- [x] 告警聚合器（AlertAggregator）
- [x] 告警去重器（AlertDeduplicator）
- [x] 告警分组器（AlertGrouper）
- [x] 静默机制（防止告警风暴）
- [x] 去重机制（避免重复告警）
- [x] 14 个单元测试通过

> **状态**：监控告警系统已 100% 完成，生产就绪！  
> **核心特性**：
> - ✅ 完整的 Prometheus 指标体系（18+ 指标）
> - ✅ Grafana Dashboard 模板
> - ✅ 实时告警规则引擎
> - ✅ 多通知渠道（Webhook/钉钉/邮件）
> - ✅ 告警聚合降噪（静默/去重/分组）
> - ✅ 系统资源监控
> - ✅ 完整的示例和文档

> **代码统计**：
> - 代码总量：~1200 行
> - 测试覆盖：14 个单元测试
> - 模块数量：5 个核心模块

#### 3.2 日志增强（完成度 100%）✅
**详细规划**：`docs/log_enhancement_plan.md`

**阶段 1：结构化日志（30%）✅**
- [x] 创建 flux-logging crate
- [x] LogEntry 结构（JSON Lines 格式）
- [x] LogEntryBuilder 构建器
- [x] 自定义字段支持
- [x] JSON 序列化

**阶段 2：日志采样（30%）✅**
- [x] LogSampler 实现
- [x] 按比例采样（Ratio）
- [x] 按级别采样（ByLevel）
- [x] 速率限制（RateLimit）
- [x] 自适应采样（Adaptive）

**阶段 3：分布式追踪（20%）✅**
- [x] TraceSpan 实现
- [x] trace_id/span_id 生成
- [x] Span 创建 API
- [x] trace_id 关联

**阶段 4：日志聚合（20%）✅**
- [x] LogAggregator 实现
- [x] 批量写入
- [x] 定期刷新
- [x] 缓冲区管理
- [x] 14 个单元测试通过

> **状态**：日志增强系统已 100% 完成，生产就绪！  
> **核心特性**：
> - ✅ 结构化日志（JSON Lines 格式）
> - ✅ 日志采样（5 种策略）
> - ✅ 分布式追踪（trace_id/span_id）
> - ✅ 日志聚合（批量写入）
> - ✅ 完整的示例和文档

> **代码统计**：
> - 代码总量：~900 行
> - 测试覆盖：14 个单元测试
> - 模块数量：4 个核心模块

#### 3.3 优雅关闭（完成度 100%）✅
**详细规划**：`docs/graceful_shutdown_plan.md`

**阶段 1：信号处理（25%）✅**
- [x] 创建 flux-shutdown crate
- [x] SignalHandler 实现
- [x] SIGTERM/SIGINT 捕获
- [x] 手动触发支持
- [x] 广播机制

**阶段 2：连接排空（25%）✅**
- [x] ConnectionTracker 实现
- [x] ConnectionGuard 实现
- [x] 超时控制
- [x] 拒绝新连接

**阶段 3：资源清理（25%）✅**
- [x] Resource trait
- [x] ResourceManager 实现
- [x] 优先级控制
- [x] 内置资源（Database/File）

**阶段 4：状态持久化（25%）✅**
- [x] StateManager 实现
- [x] 检查点机制
- [x] 原子写入
- [x] 状态恢复
- [x] 12 个单元测试通过

> **状态**：优雅关闭系统已 100% 完成，生产就绪！  
> **核心特性**：
> - ✅ 信号处理（SIGTERM/SIGINT/手动）
> - ✅ 连接排空（超时控制）
> - ✅ 资源清理（优先级管理）
> - ✅ 状态持久化（检查点机制）
> - ✅ 完整的示例和文档

> **代码统计**：
> - 代码总量：~800 行
> - 测试覆盖：12 个单元测试
> - 模块数量：5 个核心模块

### 4. 数据库连接池优化（已移除）❌

> **状态**：已移除  
> **原因**：
> - ✅ sqlx::PgPool 已提供足够的连接池功能
> - ✅ 避免重复造轮子
> - ✅ 简化项目结构
> - ✅ flux-config-manager 已直接使用 sqlx::PgPool

> **现有方案**：
> - 使用 sqlx::PgPool 的内置连接池
> - 支持最小/最大连接数配置
> - 支持连接超时和验证
> - 足够满足 IoT 平台需求

---

## 🟡 中优先级任务

### 5. 媒体功能增强

#### 5.1 HLS/FLV 完善
- [ ] TS 分片生成优化
- [ ] HTTP-FLV 流式传输
- [ ] 自适应码率（ABR）
- [ ] 多码率切换

#### 5.2 时移回放（TimeShift）
- [ ] 时移索引管理
- [ ] 快速定位和跳转
- [ ] 时移播放 API
- [ ] 时移缓存策略

#### 5.3 转码功能
- [ ] 实时转码（H264/H265）
- [ ] 音频转码（AAC/MP3）
- [ ] 分辨率调整
- [ ] 码率控制

### 6. 安全加固

#### 6.1 认证授权
- [ ] JWT Token 认证
- [ ] RBAC 权限控制
- [ ] API Key 管理
- [ ] OAuth2 集成

#### 6.2 传输安全
- [ ] TLS/SSL 支持
- [ ] 证书管理
- [ ] 加密传输（SRTP/DTLS）

#### 6.3 审计日志
- [ ] 操作审计记录
- [ ] 访问日志
- [ ] 安全事件追踪

### 7. Web UI 管理界面
- [ ] 设备管理界面
- [ ] 流管理界面
- [ ] 规则配置界面
- [ ] 监控大屏
- [ ] 日志查询界面

### 8. 容器化和部署
- [ ] Docker 镜像构建
- [ ] Docker Compose 编排
- [ ] Kubernetes 部署清单
- [ ] Helm Chart
- [ ] CI/CD 流水线

---

## 🟢 低优先级任务

### 9. 高级协议支持

#### 9.1 WebRTC 协议（完成度 0%）
- [ ] ICE/STUN/TURN 支持
- [ ] SDP 协商
- [ ] 浏览器推流/播放
- [ ] P2P 连接管理

#### 9.2 ONVIF 协议（完成度 0%）
- [ ] 设备发现（WS-Discovery）
- [ ] 设备管理接口
- [ ] PTZ 控制
- [ ] 事件订阅

### 10. 集群和扩展性

#### 10.1 负载均衡
- [ ] 流媒体负载均衡
- [ ] 会话保持
- [ ] 健康检查

#### 10.2 分布式存储
- [ ] 对象存储集成（S3/OSS/MinIO）
- [ ] 分布式缓存（Redis）
- [ ] 存储分片策略

#### 10.3 边缘节点
- [ ] 边缘推流节点
- [ ] 边缘播放节点
- [ ] 中心-边缘同步

### 11. 性能优化

#### 11.1 压力测试
- [ ] 并发推流测试
- [ ] 并发播放测试
- [ ] 长时间稳定性测试
- [ ] 性能基准测试

#### 11.2 内存优化
- [ ] 内存池管理
- [ ] 零拷贝优化扩展
- [ ] 内存泄漏检测

#### 11.3 并发优化
- [ ] 线程池调优
- [ ] 异步任务优化
- [ ] 锁竞争优化

---

## Telemetry 系统下一步可选方向

### 1. 扩展 telemetry 覆盖面
- [ ] RTSP/SRT 服务的关键事件上报
  - 流启动/停止事件（`stream/start`, `stream/stop`）
  - 连接建立/断开事件
  - 转码/编解码性能指标
- [ ] 增加更多业务事件
  - GB28181 设备上下线事件
  - 快照提取成功/失败事件
  - 时移回放请求事件

### 2. 增强可观测性
- [ ] 分布式追踪集成
  - 添加 trace ID 关联多个事件
  - OpenTelemetry 集成（trace + span）
  - 跨服务调用链追踪
- [ ] 更多 Prometheus 指标
  - 请求延迟分位数（P50/P95/P99）
  - 吞吐量指标（帧率、码率）
  - 资源使用率（CPU、内存、磁盘 I/O）
- [ ] 日志结构化增强
  - 统一日志格式（JSON Lines）
  - 关联 trace_id/span_id
  - 支持日志采样（高频日志降噪）

### 3. 运维工具与自动化
- [ ] 实时告警规则引擎
  - 基于 telemetry 事件的规则匹配
  - 支持多种通知渠道（webhook、邮件、钉钉）
  - 告警聚合与降噪（防止告警风暴）
- [ ] Dashboard 数据源接口
  - Grafana 数据源插件
  - 预定义监控面板模板
  - 实时指标查询 API
- [ ] 自动化故障诊断
  - 根据错误模式给出诊断建议
  - 关联历史故障案例
  - 自动生成故障报告

### 4. 性能优化
- [ ] Telemetry 批量上报
  - 客户端缓冲 + 批量发送
  - 降低网络开销
- [ ] 时序数据库集成
  - InfluxDB/TimescaleDB 存储长期指标
  - 支持高效的时间范围查询
- [ ] 事件流处理
  - Kafka/Pulsar 集成
  - 实时流式分析
