# FLUX IOT 未完成功能清单

> **文档版本**: v1.0  
> **生成日期**: 2026-02-22  
> **项目状态**: 核心功能完善，部分功能待实现  
> **整体完成度**: 81%

---

## 📊 总体概览

| 类别 | 完成度 | 优先级 | 预计工期 |
|------|--------|--------|---------|
| **媒体功能集成** | 75% | 🔥 高 | 2-3 周 |
| **安全加固** | 60% ✅ | 🔥 高 | 1-2 周 |
| **GB28181 协议** | 60% | 🟡 中 | 2 周 |
| **ONVIF 协议** | 40% | 🟢 低 | 2 周 |
| **Web UI 管理** | 0% | 🟡 中 | 3-4 周 |
| **容器化部署** | 0% | 🟡 中 | 1 周 |
| **WebRTC 支持** | 0% | 🟢 低 | 4-6 周 |
| **集群扩展** | 0% | 🟢 低 | 6-8 周 |

---

## 🔥 高优先级任务（生产必需）

### 1. 媒体功能集成完善

#### 1.1 HTTP-FLV 路由集成 ⚠️

**当前状态**: HTTP-FLV 服务器已实现，但未集成到 main.rs

**位置**: `crates/flux-rtmpd/src/http_flv.rs`

**待完成**：
- [ ] 在 `main.rs` 中添加 HTTP-FLV 路由
- [ ] 集成到统一 StreamManager
- [ ] 添加客户端连接追踪
- [ ] 实现断线重连机制
- [ ] 添加性能监控

**代码示例**：
```rust
// 在 main.rs 中添加
let app = Router::new()
    .route("/health", get(health))
    .route("/api/v1/rtmp/streams", get(list_streams))
    .route("/flv/:stream_id.flv", get(http_flv))  // ← 添加这行
    .with_state(state);
```

**预计工期**: 2-3 天  
**优先级**: 🔥 高

---

#### 1.2 音频 TS 封装完善 ⚠️

**当前状态**: 只处理视频，音频暂时忽略

**位置**: `crates/flux-rtmpd/src/hls_manager.rs:382`

**待完成**：
- [ ] 实现 AAC 音频 TS 封装
- [ ] PES 音频包生成
- [ ] 音视频同步处理
- [ ] 音频 PTS/DTS 计算
- [ ] 测试验证

**技术要点**：
```rust
// 需要实现
async fn process_audio_data(&self, data: Bytes, timestamp: u32) {
    // 1. AAC 解析
    // 2. PES 封装
    // 3. TS 分片
    // 4. 音视频同步
}
```

**预计工期**: 3-5 天  
**优先级**: 🔥 高

---

#### 1.3 ABR 与 HLS 实际集成 ⚠️

**当前状态**: ABR Controller 已实现，但未集成到 HLS 生成流程

**位置**: `crates/flux-media-core/src/playback/abr.rs`

**待完成**：
- [ ] 集成 AbrController 到 HlsManager
- [ ] 实现带宽估算
- [ ] 实现码率切换决策
- [ ] 生成 Master Playlist
- [ ] 客户端自适应播放测试

**集成方案**：
```rust
// HlsManager 中添加
pub struct HlsManager {
    // ...
    abr_controller: Arc<AbrController>,  // ← 添加
}

// 在生成 playlist 时
let quality = abr_controller.select_quality(bandwidth).await;
```

**预计工期**: 3-5 天  
**优先级**: 🟡 中

---

#### 1.4 多码率编码器集成 ⚠️

**当前状态**: TranscodeProcessor 已实现，但未集成到实际流程

**位置**: `crates/flux-stream/src/processor/transcode.rs`

**待完成**：
- [ ] 集成 TranscodeProcessor 到 RTMP/RTSP
- [ ] 实现多码率并行编码
- [ ] 生成多码率 HLS 输出
- [ ] 实现码率自动切换
- [ ] 性能优化和测试

**集成示例**：
```rust
// 在 StreamManager 中
async fn start_transcode(&self, stream_id: &StreamId) {
    let processor = TranscodeProcessor::new(
        stream_id.clone(),
        input_url,
        bitrates,  // 多码率配置
        hw_accel,
        output_dir,
    );
    processor.start().await?;
}
```

**预计工期**: 5-7 天  
**优先级**: 🔥 高

---

### 2. 安全加固（生产必需）✅ **已完成 60%**

> **最新更新**: 2026-02-22  
> **状态**: 已集成 flux-middleware 到 flux-rtmpd

#### 2.1 JWT Token 认证 ✅ **已完成**

**当前状态**: 已实现并集成

**已完成**：
- [x] 创建 `flux-middleware` crate（统一中间件）
- [x] JWT Token 生成和验证
- [x] Token 刷新机制
- [x] 中间件集成（Axum）
- [x] 登录 API
- [x] 集成到 flux-rtmpd

**待完成**：
- [ ] Token 黑名单（Redis）
- [ ] 登出 API
- [ ] Token 自动续期

**技术栈**：
- `jsonwebtoken` crate
- `bcrypt` 密码哈希
- Redis 存储黑名单

**代码结构**：
```rust
// crates/flux-auth/src/jwt.rs
pub struct JwtManager {
    secret: String,
    expiration: Duration,
}

impl JwtManager {
    pub fn generate_token(&self, user_id: &str) -> Result<String>;
    pub fn verify_token(&self, token: &str) -> Result<Claims>;
    pub fn refresh_token(&self, token: &str) -> Result<String>;
}

// 中间件
pub async fn auth_middleware(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode>;
```

**预计工期**: 5-7 天  
**优先级**: 🔥 高

---

#### 2.2 RBAC 权限控制 ✅ **已完成**

**当前状态**: 已实现并集成

**已完成**：
- [x] 角色定义（Admin/Operator/Viewer）
- [x] 权限定义（资源 + 操作）
- [x] 用户-角色关联
- [x] 角色-权限关联
- [x] 权限检查中间件
- [x] 集成到 flux-rtmpd API

**待完成**：
- [ ] 数据库持久化
- [ ] 动态角色管理 API

**数据模型**：
```sql
-- 用户表
CREATE TABLE users (
    id UUID PRIMARY KEY,
    username VARCHAR(50) UNIQUE,
    password_hash VARCHAR(255),
    created_at TIMESTAMP
);

-- 角色表
CREATE TABLE roles (
    id UUID PRIMARY KEY,
    name VARCHAR(50) UNIQUE,
    description TEXT
);

-- 权限表
CREATE TABLE permissions (
    id UUID PRIMARY KEY,
    resource VARCHAR(50),  -- streams, devices, rules
    action VARCHAR(20)     -- read, write, delete
);

-- 用户-角色关联
CREATE TABLE user_roles (
    user_id UUID REFERENCES users(id),
    role_id UUID REFERENCES roles(id),
    PRIMARY KEY (user_id, role_id)
);

-- 角色-权限关联
CREATE TABLE role_permissions (
    role_id UUID REFERENCES roles(id),
    permission_id UUID REFERENCES permissions(id),
    PRIMARY KEY (role_id, permission_id)
);
```

**预计工期**: 7-10 天  
**优先级**: 🔥 高

---

#### 2.3 TLS/SSL 支持 ❌

**当前状态**: 未实现

**待完成**：
- [ ] HTTPS 支持（Axum）
- [ ] RTMPS 支持（RTMP over TLS）
- [ ] RTSPS 支持（RTSP over TLS）
- [ ] 证书管理（Let's Encrypt）
- [ ] 证书自动续期
- [ ] 配置文件支持

**配置示例**：
```toml
[tls]
enabled = true
cert_path = "/etc/flux/certs/server.crt"
key_path = "/etc/flux/certs/server.key"
auto_renew = true

[https]
enabled = true
port = 443

[rtmps]
enabled = true
port = 1936

[rtsps]
enabled = true
port = 322
```

**技术栈**：
- `rustls` TLS 库
- `acme-lib` Let's Encrypt 客户端

**预计工期**: 5-7 天  
**优先级**: 🔥 高

---

#### 2.4 限流保护 ✅ **已完成**

**当前状态**: 已实现并集成

**已完成**：
- [x] 令牌桶算法实现
- [x] 按 IP 限流（100次/分钟）
- [x] 全局限流（10000次/分钟）
- [x] 按资源限流（1000客户端/流）
- [x] 限流中间件集成
- [x] 应用到流媒体路由

**待完成**：
- [ ] 带宽限流实际应用
- [ ] 动态限流配置
- [ ] 限流统计和监控

---

#### 2.5 API Key 管理 ❌

**当前状态**: 未实现

**待完成**：
- [ ] API Key 生成
- [ ] API Key 存储（数据库）
- [ ] API Key 验证中间件
- [ ] API Key 权限范围
- [ ] API Key 过期管理
- [ ] API Key CRUD API

**数据模型**：
```sql
CREATE TABLE api_keys (
    id UUID PRIMARY KEY,
    key_hash VARCHAR(255) UNIQUE,
    name VARCHAR(100),
    user_id UUID REFERENCES users(id),
    scopes TEXT[],  -- ['streams:read', 'devices:write']
    expires_at TIMESTAMP,
    created_at TIMESTAMP,
    last_used_at TIMESTAMP
);
```

**预计工期**: 3-5 天  
**优先级**: 🟡 中

---

#### 2.5 审计日志 ❌

**当前状态**: 未实现

**待完成**：
- [ ] 操作审计记录
- [ ] 访问日志记录
- [ ] 安全事件追踪
- [ ] 审计日志查询 API
- [ ] 日志归档策略
- [ ] 日志导出功能

**审计事件**：
```rust
pub enum AuditEvent {
    UserLogin { user_id: String, ip: String },
    UserLogout { user_id: String },
    StreamStart { stream_id: String, user_id: String },
    StreamStop { stream_id: String, user_id: String },
    ConfigChange { key: String, old_value: String, new_value: String },
    PermissionDenied { user_id: String, resource: String, action: String },
}
```

**预计工期**: 5-7 天  
**优先级**: 🟡 中

---

### 3. 代码质量清理

#### 3.1 修复编译警告 ⚠️

**当前状态**: 15 个编译警告

**待修复**：
- [ ] 未使用的导入（6处）
- [ ] 未使用的变量（4处）
- [ ] 未使用的字段（3处）
- [ ] 废弃函数使用（2处）

**位置**：
```
flux-media-core: 6 warnings
flux-video: 3 warnings
flux-rtmpd: 4 warnings
flux-stream: 2 warnings
```

**预计工期**: 1 天  
**优先级**: 🟡 中

---

#### 3.2 完成 TODO 标记 ⚠️

**待完成的 TODO**：

1. **插件加载器优化**
   - 位置: `flux-server/src/main.rs:204`
   - 内容: `// TODO: move to a proper loader service`
   - 优先级: 🟡 中

2. **延迟统计实现**
   - 位置: `flux-storage/src/backend/local.rs:262`
   - 内容: `avg_read_latency_ms: 0.0,  // TODO: 实现延迟统计`
   - 优先级: 🟢 低

3. **音频 TS 封装**
   - 位置: `flux-rtmpd/src/hls_manager.rs:382`
   - 内容: `// TODO: 实现音频 TS 封装`
   - 优先级: 🔥 高

4. **Accept 结果处理**
   - 位置: `flux-rtmpd/src/rtmp_server.rs:200`
   - 内容: `// TODO: 处理 accept 结果`
   - 优先级: 🟡 中

**预计工期**: 2-3 天  
**优先级**: 🟡 中

---

## 🟡 中优先级任务（功能完善）

### 4. GB28181 协议完善

**当前完成度**: 60%

**已完成**：
- ✅ SIP 信令基础
- ✅ 设备注册
- ✅ 目录查询

**待完成**：

#### 4.1 媒体流集成（RTP/PS）❌
- [ ] RTP over UDP 接收
- [ ] PS 流解析
- [ ] H264/H265 提取
- [ ] AAC 音频提取
- [ ] 媒体流转发

**预计工期**: 5-7 天  
**优先级**: 🟡 中

---

#### 4.2 PTZ 控制 ❌
- [ ] PTZ 控制命令封装
- [ ] 云台方向控制（上下左右）
- [ ] 变倍控制（放大缩小）
- [ ] 预置位管理
- [ ] 巡航轨迹

**预计工期**: 3-5 天  
**优先级**: 🟡 中

---

#### 4.3 录像回放 ❌
- [ ] 录像查询接口
- [ ] 录像回放控制
- [ ] 录像下载
- [ ] 时间轴导航
- [ ] 倍速播放

**预计工期**: 5-7 天  
**优先级**: 🟡 中

---

### 5. 时移回放功能

**当前完成度**: 0%

#### 5.1 时移索引管理 ❌
- [ ] 时移索引数据结构
- [ ] 索引文件生成
- [ ] 索引持久化
- [ ] 索引查询优化
- [ ] 索引清理策略

**数据结构**：
```rust
pub struct TimeshiftIndex {
    stream_id: StreamId,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    segments: Vec<SegmentInfo>,
}

pub struct SegmentInfo {
    sequence: u64,
    timestamp: u64,
    duration: f64,
    file_path: String,
    keyframe: bool,
}
```

**预计工期**: 3-5 天  
**优先级**: 🟡 中

---

#### 5.2 快速定位和跳转 ❌
- [ ] 时间点定位算法
- [ ] 关键帧索引
- [ ] 快速跳转实现
- [ ] 精确定位优化
- [ ] 缓存预加载

**预计工期**: 3-5 天  
**优先级**: 🟡 中

---

#### 5.3 时移播放 API ❌
- [ ] 时移播放接口
- [ ] 播放控制（播放/暂停/跳转）
- [ ] 倍速播放
- [ ] 进度查询
- [ ] 客户端集成

**API 设计**：
```rust
// GET /api/v1/timeshift/:stream_id/play?start_time=xxx&duration=xxx
// POST /api/v1/timeshift/:stream_id/seek
// GET /api/v1/timeshift/:stream_id/status
```

**预计工期**: 3-5 天  
**优先级**: 🟡 中

---

### 6. Web UI 管理界面

**当前完成度**: 0%

**技术栈建议**：
- 前端：React + TypeScript + Ant Design
- 状态管理：Redux Toolkit
- 图表：ECharts
- 视频播放：Video.js

#### 6.1 设备管理界面 ❌
- [ ] 设备列表
- [ ] 设备详情
- [ ] 设备添加/编辑/删除
- [ ] 设备状态监控
- [ ] 设备分组管理

**预计工期**: 5-7 天  
**优先级**: 🟡 中

---

#### 6.2 流管理界面 ❌
- [ ] 流列表
- [ ] 流详情
- [ ] 流启动/停止
- [ ] 流状态监控
- [ ] 流质量统计

**预计工期**: 5-7 天  
**优先级**: 🟡 中

---

#### 6.3 规则配置界面 ❌
- [ ] 规则列表
- [ ] 规则编辑器（Rhai 脚本）
- [ ] 规则测试
- [ ] 规则启用/禁用
- [ ] 规则执行历史

**预计工期**: 5-7 天  
**优先级**: 🟡 中

---

#### 6.4 监控大屏 ❌
- [ ] 实时流监控
- [ ] 系统资源监控
- [ ] 告警展示
- [ ] 统计图表
- [ ] 多屏切换

**预计工期**: 7-10 天  
**优先级**: 🟡 中

---

#### 6.5 日志查询界面 ❌
- [ ] 日志搜索
- [ ] 日志过滤
- [ ] 日志导出
- [ ] 日志统计
- [ ] 实时日志流

**预计工期**: 3-5 天  
**优先级**: 🟢 低

---

### 7. 容器化和部署

**当前完成度**: 0%

#### 7.1 Docker 镜像构建 ❌
- [ ] 多阶段构建 Dockerfile
- [ ] 镜像优化（体积压缩）
- [ ] 健康检查配置
- [ ] 环境变量配置
- [ ] 镜像推送到 Registry

**Dockerfile 示例**：
```dockerfile
# 构建阶段
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

# 运行阶段
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/flux-server /usr/local/bin/
EXPOSE 3000 1883 1935 554
CMD ["flux-server"]
```

**预计工期**: 2-3 天  
**优先级**: 🟡 中

---

#### 7.2 Docker Compose 编排 ❌
- [ ] 服务编排配置
- [ ] 数据库容器
- [ ] Redis 容器
- [ ] 网络配置
- [ ] 数据卷管理

**docker-compose.yml 示例**：
```yaml
version: '3.8'
services:
  flux-server:
    build: .
    ports:
      - "3000:3000"
      - "1883:1883"
      - "1935:1935"
      - "554:554"
    environment:
      - DATABASE_URL=postgres://postgres:password@db:5432/flux
      - REDIS_URL=redis://redis:6379
    depends_on:
      - db
      - redis
  
  db:
    image: postgres:15
    environment:
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=flux
    volumes:
      - postgres_data:/var/lib/postgresql/data
  
  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data

volumes:
  postgres_data:
  redis_data:
```

**预计工期**: 1-2 天  
**优先级**: 🟡 中

---

#### 7.3 Kubernetes 部署 ❌
- [ ] Deployment 配置
- [ ] Service 配置
- [ ] ConfigMap 配置
- [ ] Secret 配置
- [ ] Ingress 配置
- [ ] HPA 自动扩缩容

**预计工期**: 3-5 天  
**优先级**: 🟢 低

---

#### 7.4 Helm Chart ❌
- [ ] Chart 结构
- [ ] Values 配置
- [ ] Templates 编写
- [ ] Chart 打包
- [ ] Chart 发布

**预计工期**: 3-5 天  
**优先级**: 🟢 低

---

#### 7.5 CI/CD 流水线 ❌
- [ ] GitHub Actions 配置
- [ ] 自动测试
- [ ] 自动构建
- [ ] 自动部署
- [ ] 版本发布

**预计工期**: 3-5 天  
**优先级**: 🟡 中

---

## 🟢 低优先级任务（长期规划）

### 8. ONVIF 协议完善

**当前完成度**: 40%

**已完成**：
- ✅ 设备发现（WS-Discovery）
- ✅ 基础设备管理

**待完成**：

#### 8.1 PTZ 控制完善 ❌
- [ ] 绝对移动
- [ ] 相对移动
- [ ] 连续移动
- [ ] 停止命令
- [ ] 预置位管理

**预计工期**: 3-5 天  
**优先级**: 🟢 低

---

#### 8.2 事件订阅 ❌
- [ ] 事件订阅接口
- [ ] 事件通知接收
- [ ] 事件过滤
- [ ] 事件持久化
- [ ] 事件查询

**预计工期**: 3-5 天  
**优先级**: 🟢 低

---

#### 8.3 录像管理 ❌
- [ ] 录像搜索
- [ ] 录像回放
- [ ] 录像下载
- [ ] 录像删除
- [ ] 录像元数据

**预计工期**: 5-7 天  
**优先级**: 🟢 低

---

### 9. WebRTC 协议支持

**当前完成度**: 0%

#### 9.1 ICE/STUN/TURN 支持 ❌
- [ ] ICE 候选收集
- [ ] STUN 服务器集成
- [ ] TURN 服务器集成
- [ ] NAT 穿透
- [ ] 连接建立

**技术栈**：
- `webrtc-rs` crate
- coturn TURN 服务器

**预计工期**: 7-10 天  
**优先级**: 🟢 低

---

#### 9.2 SDP 协商 ❌
- [ ] Offer/Answer 生成
- [ ] SDP 解析
- [ ] 媒体协商
- [ ] 编解码器协商
- [ ] 传输参数协商

**预计工期**: 5-7 天  
**优先级**: 🟢 低

---

#### 9.3 浏览器推流/播放 ❌
- [ ] WebRTC 推流
- [ ] WebRTC 播放
- [ ] 信令服务器
- [ ] JavaScript SDK
- [ ] 示例页面

**预计工期**: 7-10 天  
**优先级**: 🟢 低

---

### 10. 集群和扩展性

**当前完成度**: 0%

#### 10.1 负载均衡 ❌
- [ ] 流媒体负载均衡算法
- [ ] 会话保持机制
- [ ] 健康检查
- [ ] 自动故障转移
- [ ] 负载监控

**预计工期**: 7-10 天  
**优先级**: 🟢 低

---

#### 10.2 分布式存储 ❌
- [ ] S3 存储集成
- [ ] OSS 存储集成
- [ ] MinIO 存储集成
- [ ] Redis 缓存集成
- [ ] 存储分片策略

**预计工期**: 7-10 天  
**优先级**: 🟢 低

---

#### 10.3 边缘节点 ❌
- [ ] 边缘推流节点
- [ ] 边缘播放节点
- [ ] 中心-边缘同步
- [ ] 边缘节点管理
- [ ] 边缘节点监控

**预计工期**: 10-14 天  
**优先级**: 🟢 低

---

## 📅 建议实施计划

### 阶段 1：生产就绪（4-6 周）

**Week 1-2: 安全加固**
- [ ] JWT 认证系统（5-7 天）
- [ ] TLS/SSL 支持（5-7 天）
- [ ] RBAC 权限控制（7-10 天）

**Week 3: 媒体功能完善**
- [ ] HTTP-FLV 路由集成（2-3 天）
- [ ] 音频 TS 封装（3-5 天）
- [ ] ABR 集成（3-5 天）

**Week 4-5: 转码功能**
- [ ] 多码率编码器集成（5-7 天）
- [ ] 实时转码集成（5-7 天）

**Week 6: 代码质量**
- [ ] 修复所有编译警告（1 天）
- [ ] 完成 TODO 标记（2-3 天）
- [ ] 补充测试（2-3 天）

---

### 阶段 2：功能完善（6-8 周）

**Week 7-8: GB28181 完善**
- [ ] 媒体流集成（5-7 天）
- [ ] PTZ 控制（3-5 天）
- [ ] 录像回放（5-7 天）

**Week 9-10: 时移回放**
- [ ] 时移索引管理（3-5 天）
- [ ] 快速定位跳转（3-5 天）
- [ ] 时移播放 API（3-5 天）

**Week 11-13: Web UI 开发**
- [ ] 设备管理界面（5-7 天）
- [ ] 流管理界面（5-7 天）
- [ ] 监控大屏（7-10 天）

**Week 14: 容器化部署**
- [ ] Docker 镜像（2-3 天）
- [ ] Docker Compose（1-2 天）
- [ ] CI/CD 流水线（3-5 天）

---

### 阶段 3：扩展增强（长期）

- ONVIF 协议完善（2 周）
- WebRTC 支持（4-6 周）
- 集群和扩展性（6-8 周）
- 性能优化（持续进行）

---

## 📊 资源需求评估

### 人力需求

| 阶段 | 角色 | 人数 | 工期 |
|------|------|------|------|
| 阶段 1 | 后端开发 | 2-3 人 | 6 周 |
| 阶段 2 | 后端开发 + 前端开发 | 3-4 人 | 8 周 |
| 阶段 3 | 后端开发 + 运维 | 2-3 人 | 持续 |

### 技能要求

**后端开发**：
- Rust 编程（必需）
- 流媒体协议（RTMP/RTSP/HLS）
- FFmpeg 使用
- 数据库设计
- 安全加固经验

**前端开发**：
- React + TypeScript
- 流媒体播放器集成
- 数据可视化
- UI/UX 设计

**运维**：
- Docker/Kubernetes
- CI/CD 流水线
- 监控告警
- 性能调优

---

## 🎯 成功标准

### 阶段 1 完成标准

- ✅ 所有 API 都有 JWT 认证
- ✅ 支持 HTTPS/RTMPS/RTSPS
- ✅ RBAC 权限控制完整
- ✅ HTTP-FLV 正常工作
- ✅ 音频视频都能正常播放
- ✅ 多码率转码正常
- ✅ 无编译警告
- ✅ 测试覆盖率 > 80%

### 阶段 2 完成标准

- ✅ GB28181 设备正常接入
- ✅ 时移回放功能正常
- ✅ Web UI 基本功能完整
- ✅ Docker 部署正常
- ✅ CI/CD 流水线运行

### 阶段 3 完成标准

- ✅ 支持 100+ 并发流
- ✅ 集群部署正常
- ✅ 分布式存储正常
- ✅ 边缘节点正常

---

## 📝 备注

1. **优先级说明**：
   - 🔥 高：生产环境必需，影响系统安全和核心功能
   - 🟡 中：重要功能，影响用户体验
   - 🟢 低：增强功能，可延后实现

2. **工期估算**：
   - 基于 1 名熟练开发者的工作量
   - 包含设计、开发、测试、文档时间
   - 实际工期可能因团队规模和经验而异

3. **依赖关系**：
   - 安全加固应优先完成
   - Web UI 依赖后端 API 完善
   - 集群功能依赖单机稳定性

4. **风险提示**：
   - WebRTC 实现复杂度高，可能超期
   - 集群功能需要大量测试验证
   - 性能优化是持续性工作

---

**文档维护**: 请在完成功能后及时更新此文档  
**最后更新**: 2026-02-22
