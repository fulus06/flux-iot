# Wasm 插件集成指南

## 📋 概述

FLUX IOT 平台已成功将 Wasm 插件系统集成到 Rule Worker 主流程中。插件在消息处理管道中扮演关键角色，提供数据预处理、协议转换、外部服务调用等能力。

## 🏗️ 架构设计

### 数据流

```
MQTT 设备消息
    ↓
EventBus (消息总线)
    ↓
Rule Worker
    ├─→ 🔥 阶段 1: 插件预处理
    │   └─→ 调用 Wasm 插件处理原始消息
    │
    ├─→ 🔥 阶段 2: 规则引擎执行
    │   └─→ Rhai 脚本评估消息
    │
    └─→ 🔥 阶段 3: 动作插件（可选）
        └─→ 规则触发后执行动作
```

### 实现位置

**文件**: `crates/flux-server/src/worker.rs`

```rust
loop {
    match rx.recv().await {
        Ok(msg) => {
            // 阶段 1: 插件预处理
            let msg_json = serde_json::to_string(&msg)?;
            match plugin_manager.call_plugin("dummy_plugin", "on_msg", &msg_json) {
                Ok(result) => { /* 处理结果 */ },
                Err(e) => { /* 插件失败不阻止规则执行 */ }
            }
            
            // 阶段 2: 规则引擎执行
            for script_id in script_ids {
                if script_engine.eval_message(&script_id, &msg)? {
                    // 阶段 3: 执行动作
                }
            }
        }
    }
}
```

## 🎯 使用场景

### 场景 1: 协议转换插件

**问题**: 设备发送 Modbus 二进制数据，规则引擎无法直接处理

**解决方案**: 创建 Modbus 解析插件

```rust
// plugins/modbus_parser/src/lib.rs
#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    let input = unsafe { read_string_from_host(ptr, len) };
    
    // 解析 Modbus 二进制数据
    let modbus_data = parse_modbus(&input);
    
    // 转换为标准 JSON
    let json = json!({
        "device_id": modbus_data.device_id,
        "registers": modbus_data.registers,
        "timestamp": modbus_data.timestamp
    });
    
    info!("Modbus data parsed: {} registers", modbus_data.registers.len());
    
    // 返回处理结果
    json.to_string().len() as i32
}
```

### 场景 2: 数据增强插件

**问题**: 需要根据设备 ID 查询地理位置和天气信息

**解决方案**: 创建数据增强插件

```rust
// plugins/data_enricher/src/lib.rs
#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    let input = unsafe { read_string_from_host(ptr, len) };
    let msg: Message = serde_json::from_str(&input).unwrap();
    
    // 查询设备位置
    let location = lookup_device_location(&msg.device_id);
    
    // 查询天气信息
    let weather = fetch_weather_api(&location);
    
    // 增强消息
    let enriched = json!({
        "original": msg,
        "location": location,
        "weather": weather
    });
    
    info!("Message enriched with location and weather data");
    
    enriched.to_string().len() as i32
}
```

### 场景 3: 动作执行插件

**问题**: 规则触发后需要发送邮件、调用 Webhook

**解决方案**: 创建动作插件

```rust
// plugins/action_handler/src/lib.rs
#[no_mangle]
pub extern "C" fn execute_action(ptr: i32, len: i32) -> i32 {
    let input = unsafe { read_string_from_host(ptr, len) };
    let msg: Message = serde_json::from_str(&input).unwrap();
    
    // 发送邮件通知
    send_email_alert(&msg);
    
    // 调用 Webhook
    call_webhook("https://api.example.com/alert", &msg);
    
    // 记录到外部系统
    log_to_external_system(&msg);
    
    info!("Actions executed successfully");
    
    1 // 成功
}
```

## 🔧 开发插件

### 1. 创建插件项目

```bash
cd plugins
cargo new --lib my_plugin
cd my_plugin
```

### 2. 配置 Cargo.toml

```toml
[package]
name = "my_plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
flux-plugin-sdk = { path = "../../sdk/flux-plugin-sdk" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "z"     # 优化体积
lto = true          # 链接时优化
strip = true        # 移除符号表
```

### 3. 实现插件逻辑

```rust
use flux_plugin_sdk::{export_plugin_alloc, read_string_from_host};
use flux_plugin_sdk::{trace, debug, info, warn, error};

export_plugin_alloc!();

#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    trace!("on_msg called");
    
    let input = unsafe { read_string_from_host(ptr, len) };
    debug!("Received {} bytes", input.len());
    
    // 你的业务逻辑
    let result = process_message(&input);
    
    info!("Message processed successfully");
    
    result
}

fn process_message(input: &str) -> i32 {
    // 实现你的处理逻辑
    input.len() as i32
}
```

### 4. 编译插件

```bash
cargo build --target wasm32-unknown-unknown --release
```

### 5. 部署插件

```bash
cp target/wasm32-unknown-unknown/release/my_plugin.wasm plugins/
```

## 📊 日志系统

插件支持 5 个级别的日志，与 Host 的 `tracing` 系统完全集成：

```rust
trace!("详细追踪信息");
debug!("调试信息");
info!("正常运行信息");
warn!("警告信息");
error!("错误信息");
```

### 日志过滤

通过环境变量控制日志级别：

```bash
# 显示所有插件日志
export RUST_LOG=wasm_plugin=trace

# 只显示警告和错误
export RUST_LOG=wasm_plugin=warn

# 生产环境配置
export RUST_LOG=info,wasm_plugin=warn
```

## 🔒 安全性

### 内存隔离

- 插件运行在独立的 Wasm 沙箱中
- 无法直接访问 Host 内存
- 所有数据通过序列化传递

### 资源限制

```rust
// 日志长度限制
const MAX_LOG_LEN: usize = 4096;

// 可以添加更多限制
// - 执行时间限制
// - 内存使用限制
// - CPU 配额限制
```

### 错误处理

插件失败不会影响主流程：

```rust
match plugin_manager.call_plugin("my_plugin", "on_msg", &msg_json) {
    Ok(result) => { /* 使用结果 */ },
    Err(e) => {
        // 记录错误但继续执行
        warn!("Plugin failed: {}, continuing", e);
    }
}
```

## 🧪 测试

### 运行集成测试

```bash
./test_plugin_integration.sh
```

### 手动测试

```bash
# 启动服务器
export RUST_LOG=debug,wasm_plugin=trace
cargo run -p flux-server

# 发送测试消息
curl -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "test/sensor",
    "payload": {"temperature": 85}
  }'
```

## 📈 性能考虑

### 插件调用开销

- Wasm 函数调用: ~100ns
- 内存序列化: ~O(n)
- 总开销: 微秒级

### 优化建议

1. **避免频繁调用**: 批量处理消息
2. **缓存结果**: 对于相同输入缓存输出
3. **异步处理**: 长时间操作使用异步模式
4. **选择性调用**: 只对需要的消息调用插件

## 🚀 下一步

### 待实现功能

1. **配置驱动**: 通过配置文件指定每个规则使用哪些插件
2. **插件链**: 支持多个插件串联处理
3. **热重载**: 支持插件热更新
4. **插件市场**: 提供常用插件库

### 示例配置（未来）

```toml
[[rules]]
name = "temperature_alert"
preprocessors = ["modbus_parser", "data_enricher"]
script = "temperature_check.rhai"
actions = ["send_email", "trigger_webhook"]
```

## 📚 参考资料

- [Wasmtime 文档](https://docs.wasmtime.dev/)
- [Wasm 规范](https://webassembly.github.io/spec/)
- [Rust Wasm Book](https://rustwasm.github.io/docs/book/)
