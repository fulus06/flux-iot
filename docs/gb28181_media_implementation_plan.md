# GB28181/多协议媒体闭环实施方案（可插拔后端 + 媒体能力模块化 + Snapshot 双模式）

## 1. 背景与目标

本方案面向 **FLUX IOT（Rust 核心服务）** 的视频子系统落地：

- 在 `flux-server` 中对外提供统一的 GB28181 管理 API（invite/bye/catalog/device-info/device-status/设备与通道查询等）。
- 支持两种部署形态并可平滑切换：
  - **方案 A（embedded）**：`flux-server` 单进程内运行 SIP 控制面。
  - **方案 B1（remote）**：SIP + RTP + 存储 + 截图由远端媒体服务承载；`flux-server` 作为网关转发同一套 HTTP API。
- 进一步面向未来多协议（GB28181/RTMP/RTSP/WebRTC 等）需求，将 **存储/截图/转码** 等能力从协议实现中剥离，形成 **协议无关的 media crate**，实现复用。
- Snapshot 同时满足两类诉求：
  - **A：Keyframe 快照**（低成本、低延迟）。
  - **B：Decode 快照**（高质量、可缩放/水印/OSD，可扩展）。
  - 对外提供统一 `mode=auto|keyframe|decode`，并具备缓存与降级策略。

> 注：`flux-server` 侧“可插拔后端（B1）”已在代码中落地，本方案在此基础上扩展到媒体闭环与模块化。

---

## 2. 当前已完成的关键实现（落地状态）

### 2.1 `flux-server` 内实现 GB28181 可插拔后端（A/B1）

- 新增模块：`crates/flux-server/src/gb28181_backend.rs`
  - `Gb28181Backend` trait：抽象 invite/bye/catalog/device-info/device-status/devices/channels 等能力。
  - `EmbeddedBackend`：包装 `flux_video::gb28181::sip::SipServer`。
  - `RemoteBackend`：使用 `reqwest` 将同路径 API 转发到远端服务。
- 配置扩展：`crates/flux-server/src/config.rs`
  - `gb28181.backend = embedded|remote`（默认 embedded，兼容旧配置）
  - `gb28181.remote.base_url`
- `AppState` 扩展：`gb28181_backend: Option<Arc<dyn Gb28181Backend>>`
- `src/api.rs`：GB28181 路由 handler 改为只调用 `state.gb28181_backend`。
- `src/main.rs`：按配置选择 embedded/remote：
  - embedded：创建并启动 `SipServer`，并订阅配置热更新，动态更新 REGISTER 鉴权配置。
  - remote：不创建本地 SIP，仅构造 `RemoteBackend`。

### 2.2 配置热更新策略

- `gb28181.enabled`、`gb28181.backend`、`gb28181.remote.base_url`、`gb28181.sip.*(bind/sip_domain/sip_id/device_expires/session_timeout)` 均纳入“禁止热更新字段”列表。

---

## 3. 总体架构与边界（建议最终形态）

目标是形成 **清晰分层**：

### 3.1 三层模型

1) **控制/网关层：`flux-server`**
- 对外暴露统一的管理 API（鉴权、审计、统一配置、路由聚合）。
- 通过 `Gb28181Backend` 选择后端：embedded 或 remote。

2) **协议接入层：协议 daemon（按协议拆进程）**
- 推荐：每协议一个服务（可按需部署/扩容）
  - `flux-gb28181d`：SIP + RTP ingest + demux/parse
  - `flux-rtmpd`：RTMP ingest + FLV demux
  - （未来）`flux-rtspd` / `flux-webrtcd`
- 这些服务负责：端口监听、协议栈、会话管理、将媒体数据转换为“通用样本（samples）”推送给媒体能力层。

3) **媒体能力层：协议无关 crate（建议新建）**
- 推荐新 crate：`flux-media-core`（或 `flux-media` / `flux-storage`）
- 负责：
  - 存储（切片/索引/保留策略）
  - Snapshot（keyframe/decode/caching/fallback）
  - （可选）转码/转封装能力的抽象层

### 3.2 关键边界原则

- **协议层负责“把字节变成样本（samples）”**
  - 例如：RTP/PS -> H264 NALU/Access Unit + PTS/DTS + keyframe 标识
- **媒体层负责“把样本变成文件/截图/索引”**
  - 例如：写 MP4/fMP4、存关键帧、生成 JPEG
- **网关层负责“鉴权/审计/统一 API/多租户/路由聚合”**

---

## 4. 远端媒体服务（B1）形态选择与实现建议

### 4.1 为什么推荐新 crate：`flux-gb28181d`

- 依赖更小、更安全：不携带 `flux-server` 的 MQTT/规则引擎/插件系统。
- 资源隔离更强：媒体写盘/截图不会影响平台控制面延迟。
- 易扩容：多实例部署，按摄像头数量水平扩展。

### 4.2 `flux-gb28181d` 的职责

- 对外提供与 `flux-server` 一致的 GB28181 管理 API（同路径）：
  - `POST /api/v1/gb28181/invite`
  - `POST /api/v1/gb28181/bye`
  - `POST /api/v1/gb28181/catalog`
  - `POST /api/v1/gb28181/device-info`
  - `POST /api/v1/gb28181/device-status`
  - `GET  /api/v1/gb28181/devices`
  - `GET  /api/v1/gb28181/devices/:device_id`
  - `GET  /api/v1/gb28181/devices/:device_id/channels`
- 在进程内组装：
  - `flux-video`（SIP + RTP receiver + PS/H264 解析）
  - `flux-media-core`（存储 + snapshot）

### 4.3 与 `flux-server` 的协作

- `flux-server` 配置 `gb28181.backend=remote` 后，`RemoteBackend` 只负责转发请求。
- 远端服务需要保证 JSON 字段一致。

> 建议：抽离 DTO 到小 crate（例如 `flux-gb28181-api`），供 `flux-server` 的 `RemoteBackend` 与 `flux-gb28181d` 共享，避免字段漂移。

---

## 5. 存储与 Snapshot 的协议无关化（media crate 设计）

### 5.1 新 crate 建议：`flux-media-core`

建议职责：

- `storage`：
  - 写盘 pipeline（多 worker）
  - 目录布局与文件命名
  - 分片策略（按时间、按大小、按 GOP）
  - 索引（关键帧索引、时间索引）
  - 保留策略（retention）
- `snapshot`：
  - keyframe 快照
  - decode 快照（可选高级）
  - cache
  - fallback/orchestration
- `types`：
  - `StreamId`（协议无关主键）
  - `VideoSample` / `AudioSample` / `Keyframe` 等

### 5.2 `StreamId` 统一命名

必须使用协议无关、可扩展的命名方案，例如：

- `gb28181/{device_id}/{channel_id}`
- `rtmp/{app}/{stream_name}`
- `rtsp/{host}/{path}`

目录布局建议：

- `${root}/${stream_id}/segments/...`
- `${root}/${stream_id}/keyframes/...`
- `${root}/${stream_id}/snapshots/...`

### 5.3 写盘接口（建议抽象）

建议抽象为 sink：

- `trait VideoSink { async fn push_sample(&self, stream_id: &StreamId, sample: VideoSample) -> Result<()>; }`
- 存储侧负责落盘/索引/缓存。

---

## 6. Snapshot：同时支持 Keyframe + Decode 的最佳方案（Auto + Fallback）

### 6.1 对外统一能力：`SnapshotMode`

- `auto`（默认）：优先 cache -> keyframe -> decode
- `keyframe`：仅 keyframe
- `decode`：仅 decode（强制高质量）

建议请求字段：

- `stream_id`
- `mode=auto|keyframe|decode`
- `max_age_ms`（允许返回缓存快照）
- `timeout_ms`（避免 decode 阻塞）
- 可选 `width/height`（decode 可保证高质量缩放）

### 6.2 Provider 链 + Orchestrator

在 `flux-media-core` 里实现：

- `SnapshotCache`：缓存最近一次成功快照（按 `stream_id`）
- `KeyframeSnapshotProvider`：从关键帧缓存/文件生成 JPEG
- `DecodeSnapshotProvider`：解码生成 JPEG/PNG（可用 CPU 或平台硬件加速）
- `SnapshotOrchestrator`：按模式选择 provider，或在 `auto` 下按优先级 fallback

`auto` 执行顺序建议：

1. cache 命中且不超过 `max_age_ms`：直接返回
2. keyframe provider：快速返回（低成本）
3. decode provider：超时/失败则返回错误或降级

### 6.3 并发与隔离

必须实现以下保护，避免 decode 把系统拖垮：

- 每 `stream_id` 一个 `Semaphore(1)`，避免同一路流并发 decode。
- decode 强制 `timeout`。
- decode 成功结果写 cache，后续请求秒回。

### 6.4 Decode Provider 的阶段性目标

- **阶段 D1（推荐先做）**：只解码“最新画面”，用于实时截图。
- **阶段 D2（后续增强）**：支持“按时间点截图”，依赖更完整的索引与回放能力。

---

## 7. 媒体闭环（GB28181）的落地步骤（推荐路线图）

### 阶段 0：控制面与配置（已完成）
- 可插拔后端、鉴权配置、多源配置加载、热更新策略、审计、GB28181 管理 API。

### 阶段 1：先跑通 embedded 闭环（最小可用）
- 在 `flux-server` embedded 模式下：
  - INVITE -> RTP receiver -> PS/H264 parse -> keyframe 缓存 -> snapshot API
  - 存储先支持最简路径（例如按 stream_id 写文件）
- 指标：
  - 在线设备数、活跃会话数、RTP 包计数、写盘速率、快照请求耗时/命中率

### 阶段 2：抽离 `flux-media-core`
- 从 `flux-video` 中迁移 storage/keyframe/snapshot 相关代码到 `flux-media-core`
- `flux-video` 只负责协议、接收、解析

### 阶段 3：实现 `flux-gb28181d`（远端 B1）
- 进程内组装 SIP+RTP + media-core
- 对外暴露与 `flux-server` 相同的 `/api/v1/gb28181/*`
- `flux-server` 使用 `gb28181.backend=remote` 即可切换

### 阶段 4：扩展到 RTMP 等协议
- 新增 `flux-rtmpd`：RTMP ingest + demux -> media-core
- 存储/截图完全复用

---

## 8. 配置与热更新策略（建议补充项）

### 8.1 `flux-server` 侧

- `gb28181.enabled`、`gb28181.backend`、`gb28181.remote.base_url`：禁止热更新
- `gb28181.sip.*`（bind/identity/socket）：禁止热更新
- 仅允许热更新：
  - REGISTER digest 相关配置（mode/password/per-device-passwords）

### 8.2 `flux-gb28181d` / `flux-media-core` 侧

建议新增：

- `media.storage.root_dir`
- `media.storage.segment_strategy`
- `media.storage.retention_days`
- `media.snapshot.cache_ttl_ms`
- `media.snapshot.decode.max_concurrency`

热更新建议：

- 写盘路径/端口等基础项禁止热更新
- snapshot cache TTL、decode 并发上限可以热更新

---

## 9. API 与鉴权建议

- 网关（`flux-server`）对外仍建议使用 `FLUX_ADMIN_TOKEN`（Bearer），默认不配置即拒绝访问。
- 远端媒体服务建议：
  - 内网部署 + 防火墙隔离，或
  - 独立 token（例如 `FLUX_GB28181D_TOKEN`）
- 远端 API 建议加：
  - 超时、重试策略（仅幂等请求可重试）
  - 请求链路追踪（`traceparent`）

---

## 10. 可观测性（tracing + metrics）

建议指标：

- `gb28181_devices_online`
- `gb28181_sessions_active`
- `rtp_packets_total` / `rtp_bytes_total`
- `snapshot_requests_total{mode,hit}`
- `snapshot_latency_ms{mode}`
- `storage_write_bytes_total`
- `storage_errors_total`

建议日志：

- 关键错误必须带 `stream_id/device_id/channel_id/call_id`
- snapshot provider 的降级路径应有 debug 日志（避免过量 info）

---

## 11. 测试策略

- 单元测试：
  - 配置映射、热更新策略拒绝列表
  - backend remote 转发错误映射（404/500）
- 集成测试：
  - embedded：mock RTP 输入或用最小 demo 流跑 snapshot
  - remote：起一个 test server 模拟远端 API，验证 `RemoteBackend`

---

## 12. 风险与规避

- 资源争用：decode/snapshot/写盘必须隔离并发、超时和缓存。
- API 漂移：建议抽 DTO 到 `flux-gb28181-api` crate。
- 多协议并存：以 `stream_id` 为统一主键，存储与 snapshot 完全协议无关。

---

## 13. 下一步行动清单（建议）

1. 明确 `flux-media-core` crate 的最小 API（VideoSample + StorageSink + SnapshotOrchestrator）。
2. 在 embedded 路径先跑通 keyframe snapshot（D1：最新画面）。
3. 创建 `flux-gb28181d`：复用 `flux-video` + `flux-media-core`，对外暴露同路径 API。
4. `flux-server` 配置切换到 remote 进行联调。
5. 再扩展 RTMP（`flux-rtmpd`）复用同一 media-core。
