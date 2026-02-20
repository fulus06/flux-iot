# SRT 协议实现规划

**日期**: 2026-02-20  
**当前完成度**: 0%  
**目标完成度**: 100%  
**预计工期**: 4-6 周

---

## 📊 SRT 协议概述

### 什么是 SRT？

**SRT (Secure Reliable Transport)** 是一种开源的视频传输协议，由 Haivision 开发，专为低延迟、高质量的视频流传输设计。

### 核心特性

1. **低延迟传输**
   - 端到端延迟可低至 120ms
   - 适合实时直播场景

2. **可靠传输**
   - 基于 UDP，但提供 TCP 级别的可靠性
   - ARQ（自动重传请求）机制
   - FEC（前向纠错）支持

3. **加密传输**
   - AES-128/256 加密
   - 保护视频内容安全

4. **网络自适应**
   - 动态带宽估计
   - 拥塞控制
   - 抗丢包能力强

5. **防火墙友好**
   - 支持 NAT 穿透
   - Rendezvous 模式

---

## 🎯 实现目标

### 阶段 1：基础功能（40%）
- [ ] SRT 握手协议（Handshake）
- [ ] 基本数据传输（发送/接收）
- [ ] 连接管理（Caller/Listener/Rendezvous）
- [ ] 基础错误处理

### 阶段 2：可靠性保障（30%）
- [ ] ARQ 重传机制
- [ ] 丢包检测和恢复
- [ ] 包序列号管理
- [ ] 接收缓冲区管理

### 阶段 3：性能优化（20%）
- [ ] 拥塞控制算法
- [ ] 动态带宽估计
- [ ] 延迟控制
- [ ] 流量整形

### 阶段 4：高级特性（10%）
- [ ] AES 加密/解密
- [ ] FEC 前向纠错
- [ ] 统计信息收集
- [ ] Telemetry 集成

---

## 🏗️ 技术架构

### 1. 核心模块

```
crates/flux-srt/
├── src/
│   ├── main.rs                 # 服务入口
│   ├── lib.rs                  # 库导出
│   ├── handshake.rs            # SRT 握手协议
│   ├── packet.rs               # SRT 包结构
│   ├── socket.rs               # SRT Socket 抽象
│   ├── sender.rs               # 发送端逻辑
│   ├── receiver.rs             # 接收端逻辑
│   ├── buffer.rs               # 发送/接收缓冲区
│   ├── congestion.rs           # 拥塞控制
│   ├── crypto.rs               # 加密/解密
│   ├── statistics.rs           # 统计信息
│   └── telemetry.rs            # Telemetry 客户端
├── tests/
│   ├── handshake_tests.rs
│   ├── packet_tests.rs
│   └── integration_tests.rs
└── Cargo.toml
```

### 2. SRT 包结构

```rust
// SRT 数据包类型
enum SrtPacketType {
    Data,           // 数据包
    Control,        // 控制包
}

// 控制包类型
enum SrtControlType {
    Handshake,      // 握手
    KeepAlive,      // 保活
    Ack,            // 确认
    Nak,            // 否定确认（丢包通知）
    Shutdown,       // 关闭连接
    AckAck,         // 确认的确认
    // ... 其他控制类型
}

// SRT 数据包
struct SrtPacket {
    header: SrtHeader,
    payload: Bytes,
}

// SRT 包头
struct SrtHeader {
    is_control: bool,
    packet_seq: u32,
    timestamp: u32,
    destination_socket_id: u32,
    // ... 其他字段
}
```

### 3. 连接模式

#### Caller 模式（客户端）
```rust
let socket = SrtSocket::new();
socket.connect("srt://server:9000").await?;
socket.send(data).await?;
```

#### Listener 模式（服务器）
```rust
let listener = SrtListener::bind("0.0.0.0:9000").await?;
while let Ok(socket) = listener.accept().await {
    tokio::spawn(async move {
        handle_connection(socket).await;
    });
}
```

#### Rendezvous 模式（对等连接）
```rust
let socket = SrtSocket::new();
socket.rendezvous("0.0.0.0:9000", "peer:9000").await?;
```

---

## 📋 详细实现计划

### 阶段 1：基础功能（预计 2 周）

#### 1.1 SRT 握手协议
**任务**：
- [ ] 实现 SRT 握手包结构
- [ ] 实现握手状态机
  - INDUCTION（引导阶段）
  - CONCLUSION（结论阶段）
- [ ] 支持版本协商
- [ ] 支持扩展字段

**关键点**：
- SRT 握手是 4 次握手（类似 TCP，但更复杂）
- 需要交换 Socket ID、MTU、延迟等参数

**参考**：
- RFC: SRT Protocol Specification
- libsrt 源码

#### 1.2 基本数据传输
**任务**：
- [ ] 实现 SRT 数据包封装
- [ ] 实现数据包发送逻辑
- [ ] 实现数据包接收逻辑
- [ ] 实现包序列号管理

**关键点**：
- 序列号是 31 位（最高位用于标识控制包）
- 时间戳是微秒级

#### 1.3 连接管理
**任务**：
- [ ] 实现 Caller 模式
- [ ] 实现 Listener 模式
- [ ] 实现连接状态管理
- [ ] 实现 KeepAlive 机制

**关键点**：
- 需要定期发送 KeepAlive 包
- 超时检测和自动断开

---

### 阶段 2：可靠性保障（预计 1.5 周）

#### 2.1 ARQ 重传机制
**任务**：
- [ ] 实现 NAK（否定确认）机制
- [ ] 实现重传队列
- [ ] 实现重传超时（RTO）计算
- [ ] 实现快速重传

**关键点**：
- SRT 使用选择性重传（Selective Repeat）
- NAK 包包含丢失的包序列号列表

#### 2.2 丢包检测和恢复
**任务**：
- [ ] 实现包序列号间隙检测
- [ ] 实现丢包统计
- [ ] 实现丢包率计算
- [ ] 实现自适应重传策略

**关键点**：
- 接收端检测序列号间隙
- 发送 NAK 通知发送端重传

#### 2.3 缓冲区管理
**任务**：
- [ ] 实现发送缓冲区（Send Buffer）
- [ ] 实现接收缓冲区（Receive Buffer）
- [ ] 实现缓冲区溢出处理
- [ ] 实现缓冲区大小自适应

**关键点**：
- 发送缓冲区用于重传
- 接收缓冲区用于乱序包重组

---

### 阶段 3：性能优化（预计 1.5 周）

#### 3.1 拥塞控制
**任务**：
- [ ] 实现 AIMD（加性增乘性减）算法
- [ ] 实现慢启动（Slow Start）
- [ ] 实现拥塞避免（Congestion Avoidance）
- [ ] 实现快速恢复（Fast Recovery）

**关键点**：
- SRT 使用类似 TCP 的拥塞控制
- 需要维护拥塞窗口（CWND）

#### 3.2 动态带宽估计
**任务**：
- [ ] 实现 RTT（往返时延）测量
- [ ] 实现带宽估计算法
- [ ] 实现发送速率调整
- [ ] 实现流量平滑

**关键点**：
- 使用 ACK 包计算 RTT
- 根据 RTT 和丢包率调整发送速率

#### 3.3 延迟控制
**任务**：
- [ ] 实现延迟目标设置
- [ ] 实现延迟测量
- [ ] 实现延迟补偿
- [ ] 实现 TsbPd（时间戳播放延迟）

**关键点**：
- TsbPd 是 SRT 的核心特性
- 保证接收端按时间戳顺序播放

---

### 阶段 4：高级特性（预计 1 周）

#### 4.1 AES 加密
**任务**：
- [ ] 集成 AES-128/256 加密库
- [ ] 实现密钥交换
- [ ] 实现加密/解密逻辑
- [ ] 实现密钥更新

**关键点**：
- 使用 Rust 的 `aes` 和 `ctr` crate
- 密钥通过握手交换

#### 4.2 统计信息
**任务**：
- [ ] 实现实时统计收集
- [ ] 实现统计信息查询 API
- [ ] 实现 Telemetry 集成
- [ ] 实现 Prometheus 指标导出

**统计指标**：
- 发送/接收字节数
- 发送/接收包数
- 丢包数和丢包率
- RTT（往返时延）
- 带宽利用率
- 重传次数

---

## 🔧 技术选型

### 依赖库

```toml
[dependencies]
tokio = { version = "1.35", features = ["full"] }
bytes = "1.5"
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"

# 加密
aes = "0.8"
ctr = "0.9"
rand = "0.8"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 测试
tokio-test = "0.4"
```

### 性能考虑

1. **零拷贝**：使用 `bytes::Bytes` 避免内存拷贝
2. **异步 I/O**：使用 Tokio 异步运行时
3. **无锁数据结构**：使用 `crossbeam` 或 `parking_lot`
4. **内存池**：复用缓冲区，减少分配

---

## 🧪 测试策略

### 单元测试
- [ ] 握手协议测试
- [ ] 包解析测试
- [ ] 序列号管理测试
- [ ] 缓冲区测试
- [ ] 加密/解密测试

### 集成测试
- [ ] 端到端连接测试
- [ ] 数据传输测试
- [ ] 丢包恢复测试
- [ ] 拥塞控制测试

### 性能测试
- [ ] 吞吐量测试
- [ ] 延迟测试
- [ ] 丢包率测试
- [ ] 并发连接测试

### 兼容性测试
- [ ] 与 libsrt 互操作测试
- [ ] 与 FFmpeg SRT 互操作测试
- [ ] 与 OBS SRT 互操作测试

---

## 📊 里程碑

### Milestone 1：基础连接（2 周）
- ✅ 完成握手协议
- ✅ 完成基本数据传输
- ✅ 完成 Caller/Listener 模式
- **目标**：能够建立 SRT 连接并传输数据

### Milestone 2：可靠传输（3.5 周）
- ✅ 完成 ARQ 重传
- ✅ 完成丢包检测
- ✅ 完成缓冲区管理
- **目标**：实现可靠的数据传输

### Milestone 3：性能优化（5 周）
- ✅ 完成拥塞控制
- ✅ 完成带宽估计
- ✅ 完成延迟控制
- **目标**：达到低延迟、高吞吐量

### Milestone 4：生产就绪（6 周）
- ✅ 完成加密支持
- ✅ 完成统计信息
- ✅ 完成测试覆盖
- ✅ 完成文档
- **目标**：生产环境可用

---

## 🎯 成功标准

### 功能完整性
- ✅ 支持 Caller/Listener/Rendezvous 模式
- ✅ 支持可靠传输（ARQ）
- ✅ 支持拥塞控制
- ✅ 支持 AES 加密
- ✅ 支持统计信息

### 性能指标
- **延迟**：< 200ms（端到端）
- **吞吐量**：> 100 Mbps（千兆网络）
- **丢包恢复**：< 1% 丢包率下正常工作
- **并发连接**：> 100 个连接

### 兼容性
- ✅ 与 libsrt 互操作
- ✅ 与 FFmpeg 互操作
- ✅ 与 OBS 互操作

---

## 📚 参考资料

### 官方文档
- [SRT Protocol Specification](https://github.com/Haivision/srt/blob/master/docs/API/API.md)
- [SRT Technical Overview](https://github.com/Haivision/srt/blob/master/docs/features/srt-tech-overview.md)

### 开源实现
- [libsrt (C++)](https://github.com/Haivision/srt) - 官方实现
- [srt-rs (Rust)](https://github.com/russelltg/srt-rs) - Rust 实现参考

### 相关协议
- UDT (UDP-based Data Transfer) - SRT 的前身
- QUIC - 类似的可靠 UDP 协议

---

## 🚀 快速开始（实现后）

### 发送端（Caller）
```rust
use flux_srt::SrtSocket;

#[tokio::main]
async fn main() -> Result<()> {
    let socket = SrtSocket::new();
    socket.connect("srt://server:9000").await?;
    
    // 发送数据
    let data = b"Hello, SRT!";
    socket.send(data).await?;
    
    Ok(())
}
```

### 接收端（Listener）
```rust
use flux_srt::SrtListener;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = SrtListener::bind("0.0.0.0:9000").await?;
    
    while let Ok(socket) = listener.accept().await {
        tokio::spawn(async move {
            let mut buf = vec![0u8; 1500];
            while let Ok(n) = socket.recv(&mut buf).await {
                println!("Received {} bytes", n);
            }
        });
    }
    
    Ok(())
}
```

---

## 📝 注意事项

### 技术挑战
1. **复杂的握手协议**：SRT 握手比 TCP 更复杂
2. **精确的时间控制**：需要微秒级时间戳
3. **高性能要求**：需要优化内存和 CPU 使用
4. **互操作性**：需要与 libsrt 完全兼容

### 风险评估
- **高**：握手协议实现复杂，容易出错
- **中**：拥塞控制算法需要仔细调优
- **低**：加密功能可以使用成熟的库

### 缓解措施
- 参考 libsrt 源码
- 编写详细的单元测试
- 与 libsrt 进行互操作测试
- 逐步实现，先保证基本功能

---

## 🎉 总结

SRT 协议实现是一个**中等复杂度**的项目，预计需要 **4-6 周**完成。

### 优先级排序
1. **P0（必须）**：握手、基本传输、ARQ 重传
2. **P1（重要）**：拥塞控制、延迟控制、统计信息
3. **P2（可选）**：加密、FEC、Rendezvous 模式

### 建议实施顺序
1. 先实现 Caller/Listener 模式
2. 再实现可靠传输（ARQ）
3. 然后优化性能（拥塞控制）
4. 最后添加高级特性（加密）

---

**下一步行动**：
1. 创建 `crates/flux-srt` 目录结构
2. 实现 SRT 包结构和解析
3. 实现握手协议
4. 编写单元测试

**预计开始时间**：待定  
**负责人**：FLUX IOT Team
