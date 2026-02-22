# flux-control

设备控制功能包，提供完整的设备远程控制能力。

## 功能特性

### 已实现 ✅

- ✅ **设备指令模型** - 完整的指令定义和状态管理
- ✅ **指令队列** - 高效的指令队列管理
- ✅ **指令执行器** - 异步指令执行和超时处理
- ✅ **指令通道** - 可扩展的指令下发通道
- ✅ **响应处理** - 灵活的响应处理机制

### 待实现 ⏳

- ⏳ **MQTT 通道** - MQTT 协议指令下发
- ⏳ **HTTP 通道** - HTTP 协议指令下发
- ⏳ **批量控制** - 批量设备控制
- ⏳ **场景联动** - 自动化场景执行
- ⏳ **数据持久化** - 指令历史存储

## 快速开始

### 基本使用

```rust
use flux_control::{CommandExecutor, DeviceCommand, CommandType};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建指令通道（这里使用 Mock）
    let channel = Arc::new(MockCommandChannel::new());
    
    // 创建执行器
    let executor = CommandExecutor::new(channel);
    
    // 创建指令
    let command = DeviceCommand::new(
        "device_001".to_string(),
        CommandType::SetState { state: true },
    );
    
    // 提交指令
    let command_id = executor.submit(command).await?;
    println!("Command submitted: {}", command_id);
    
    Ok(())
}
```

### 指令类型

```rust
// 通用指令
CommandType::Reboot
CommandType::Reset
CommandType::Update

// 摄像头指令
CommandType::StartStream
CommandType::StopStream
CommandType::TakeSnapshot
CommandType::PTZControl { pan: 10, tilt: 20, zoom: 5 }

// 传感器指令
CommandType::ReadValue
CommandType::SetSamplingRate { rate: 1000 }

// 执行器指令
CommandType::SetState { state: true }
CommandType::SetValue { value: 25.5 }

// 自定义指令
CommandType::Custom {
    name: "custom_command".to_string(),
    params: serde_json::json!({"key": "value"}),
}
```

### 指令状态

```rust
pub enum CommandStatus {
    Pending,    // 待发送
    Sent,       // 已发送
    Executing,  // 执行中
    Success,    // 成功
    Failed,     // 失败
    Timeout,    // 超时
    Cancelled,  // 已取消
}
```

## 架构设计

```
flux-control/
├── command/
│   ├── model.rs      # 指令模型定义
│   ├── executor.rs   # 指令执行器
│   ├── queue.rs      # 指令队列
│   └── status.rs     # 状态定义
├── channel/
│   ├── trait_def.rs  # 通道 trait
│   └── mqtt.rs       # MQTT 通道实现
└── response/
    └── handler.rs    # 响应处理器
```

## 依赖

```toml
[dependencies]
flux-control = { path = "../flux-control" }
```

## 特性标志

```toml
[features]
default = []
persistence = ["sea-orm"]  # 启用数据持久化
mqtt = ["rumqttc"]         # 启用 MQTT 通道
```

## 测试

```bash
# 运行测试
cargo test -p flux-control

# 运行特定测试
cargo test -p flux-control test_create_command
```

## 示例

查看 `examples/` 目录获取更多示例。

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！
