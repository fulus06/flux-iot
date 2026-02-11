# Wasm 插件开发指南

本指南将帮助你为 FLUX IOT 平台开发 Wasm 插件。

---

## 📋 目录

- [概述](#概述)
- [环境准备](#环境准备)
- [快速开始](#快速开始)
- [插件 SDK](#插件-sdk)
- [开发示例](#开发示例)
- [最佳实践](#最佳实践)
- [调试技巧](#调试技巧)
- [常见问题](#常见问题)

---

## 概述

### 什么是 Wasm 插件？

Wasm (WebAssembly) 插件是运行在沙箱环境中的可执行模块，用于扩展 FLUX IOT 平台的功能。插件可以：

- **数据预处理**: 协议转换、数据清洗、格式化
- **数据增强**: 添加地理位置、天气信息等
- **外部调用**: 发送邮件、调用 Webhook、访问第三方 API
- **自定义算法**: 加密、压缩、图像处理等

### 为什么使用 Wasm？

- ✅ **安全隔离**: 沙箱执行，无法访问系统资源
- ✅ **高性能**: 接近原生代码的执行速度
- ✅ **跨平台**: 一次编译，到处运行
- ✅ **多语言支持**: Rust、C、C++、AssemblyScript 等

---

## 环境准备

### 1. 安装 Rust

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 验证安装
rustc --version
cargo --version
```

### 2. 添加 Wasm 目标

```bash
# 添加 wasm32-unknown-unknown 目标
rustup target add wasm32-unknown-unknown

# 验证
rustup target list | grep wasm32-unknown-unknown
```

### 3. 安装工具（可选）

```bash
# wasm-opt: 优化 Wasm 文件大小
cargo install wasm-opt

# wasm-strip: 移除调试信息
cargo install wasm-strip
```

---

## 快速开始

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
crate-type = ["cdylib"]  # 生成动态库

[dependencies]
flux-plugin-sdk = { path = "../../sdk/flux-plugin-sdk" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "z"     # 优化体积
lto = true          # 链接时优化
strip = true        # 移除符号表
panic = "abort"     # 减小体积
```

### 3. 编写插件代码

```rust
// src/lib.rs
use flux_plugin_sdk::{export_plugin_alloc, read_string_from_host};
use flux_plugin_sdk::{info, warn, error};
use serde::{Deserialize, Serialize};

// 导出内存分配函数
export_plugin_alloc!();

// 定义数据结构
#[derive(Deserialize)]
struct InputMessage {
    device_id: String,
    temperature: f64,
}

#[derive(Serialize)]
struct OutputMessage {
    device_id: String,
    temperature: f64,
    status: String,
}

// 插件入口函数
#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    // 读取输入数据
    let input = unsafe { read_string_from_host(ptr, len) };
    
    info!("Processing message: {} bytes", input.len());
    
    // 解析 JSON
    let msg: InputMessage = match serde_json::from_str(&input) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to parse JSON: {}", e);
            return 0;
        }
    };
    
    // 业务逻辑
    let status = if msg.temperature > 30.0 {
        warn!("High temperature: {}°C", msg.temperature);
        "high"
    } else {
        "normal"
    };
    
    // 构造输出
    let output = OutputMessage {
        device_id: msg.device_id,
        temperature: msg.temperature,
        status: status.to_string(),
    };
    
    // 序列化输出
    let output_json = serde_json::to_string(&output).unwrap();
    info!("Output: {}", output_json);
    
    // 返回处理结果（这里返回长度作为示例）
    output_json.len() as i32
}
```

### 4. 编译插件

```bash
cargo build --target wasm32-unknown-unknown --release
```

编译后的文件位于：
```
target/wasm32-unknown-unknown/release/my_plugin.wasm
```

### 5. 部署插件

```bash
# 复制到插件目录
cp target/wasm32-unknown-unknown/release/my_plugin.wasm ../../plugins/

# 重启服务器
cargo run -p flux-server
```

---

## 插件 SDK

### 核心宏

#### export_plugin_alloc!()

导出内存分配和释放函数，**必须**在每个插件中调用。

```rust
use flux_plugin_sdk::export_plugin_alloc;

export_plugin_alloc!();
```

### 内存管理

#### read_string_from_host

从 Host 内存读取字符串。

```rust
use flux_plugin_sdk::read_string_from_host;

#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    let input = unsafe { read_string_from_host(ptr, len) };
    // 使用 input...
    0
}
```

**Safety**: 
- `ptr` 必须指向有效的内存地址
- `len` 必须是正确的字节长度
- 内存由 Host 管理，插件不应释放

### 日志系统

插件支持 5 个级别的日志，输出到 Host 的 `tracing` 系统。

#### trace!

详细追踪信息，通常只在开发环境启用。

```rust
use flux_plugin_sdk::trace;

trace!("Function called with param: {}", value);
```

#### debug!

调试信息，帮助理解程序执行流程。

```rust
use flux_plugin_sdk::debug;

debug!("Processing data: {:?}", data);
```

#### info!

正常的运行时信息，记录重要的业务事件。

```rust
use flux_plugin_sdk::info;

info!("Device connected: {}", device_id);
```

#### warn!

警告信息，表示潜在问题但不影响正常运行。

```rust
use flux_plugin_sdk::warn;

warn!("Temperature high: {}°C", temp);
```

#### error!

错误信息，表示严重问题需要关注。

```rust
use flux_plugin_sdk::error;

error!("Failed to parse data: {}", err);
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

---

## 开发示例

### 示例 1: 协议转换插件

将 Modbus 数据转换为标准 JSON 格式。

```rust
use flux_plugin_sdk::{export_plugin_alloc, read_string_from_host, info, error};
use serde::{Deserialize, Serialize};

export_plugin_alloc!();

#[derive(Deserialize)]
struct ModbusData {
    device_id: String,
    registers: Vec<u16>,
}

#[derive(Serialize)]
struct StandardData {
    device_id: String,
    temperature: f64,
    pressure: f64,
    timestamp: i64,
}

#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    let input = unsafe { read_string_from_host(ptr, len) };
    
    let modbus: ModbusData = match serde_json::from_str(&input) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to parse Modbus data: {}", e);
            return 0;
        }
    };
    
    // 转换 Modbus 寄存器值
    let temperature = (modbus.registers[0] as f64) / 10.0;
    let pressure = (modbus.registers[1] as f64) / 100.0;
    
    let output = StandardData {
        device_id: modbus.device_id,
        temperature,
        pressure,
        timestamp: chrono::Utc::now().timestamp(),
    };
    
    info!("Converted Modbus data: temp={}, pressure={}", temperature, pressure);
    
    1 // 成功
}
```

### 示例 2: 数据验证插件

验证传感器数据的合法性。

```rust
use flux_plugin_sdk::{export_plugin_alloc, read_string_from_host, warn, error};
use serde::Deserialize;

export_plugin_alloc!();

#[derive(Deserialize)]
struct SensorData {
    temperature: f64,
    humidity: f64,
    pressure: f64,
}

#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    let input = unsafe { read_string_from_host(ptr, len) };
    
    let data: SensorData = match serde_json::from_str(&input) {
        Ok(d) => d,
        Err(e) => {
            error!("Invalid JSON: {}", e);
            return 0;
        }
    };
    
    // 验证温度范围
    if data.temperature < -50.0 || data.temperature > 100.0 {
        warn!("Temperature out of range: {}°C", data.temperature);
        return 0;
    }
    
    // 验证湿度范围
    if data.humidity < 0.0 || data.humidity > 100.0 {
        warn!("Humidity out of range: {}%", data.humidity);
        return 0;
    }
    
    // 验证气压范围
    if data.pressure < 800.0 || data.pressure > 1200.0 {
        warn!("Pressure out of range: {} hPa", data.pressure);
        return 0;
    }
    
    1 // 验证通过
}
```

### 示例 3: 数据聚合插件

聚合多个传感器的数据。

```rust
use flux_plugin_sdk::{export_plugin_alloc, read_string_from_host, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

export_plugin_alloc!();

static mut SENSOR_DATA: Option<HashMap<String, f64>> = None;

#[derive(Deserialize)]
struct SensorReading {
    sensor_id: String,
    value: f64,
}

#[derive(Serialize)]
struct AggregatedData {
    count: usize,
    average: f64,
    min: f64,
    max: f64,
}

#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    let input = unsafe { read_string_from_host(ptr, len) };
    
    let reading: SensorReading = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    
    unsafe {
        if SENSOR_DATA.is_none() {
            SENSOR_DATA = Some(HashMap::new());
        }
        
        if let Some(ref mut data) = SENSOR_DATA {
            data.insert(reading.sensor_id, reading.value);
            
            let values: Vec<f64> = data.values().copied().collect();
            let count = values.len();
            let sum: f64 = values.iter().sum();
            let average = sum / count as f64;
            let min = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            
            info!("Aggregated {} sensors: avg={:.2}, min={:.2}, max={:.2}", 
                  count, average, min, max);
        }
    }
    
    1
}
```

---

## 最佳实践

### 1. 错误处理

始终处理错误，不要使用 `unwrap()` 或 `expect()`。

```rust
// ❌ 不好的做法
let data: MyData = serde_json::from_str(&input).unwrap();

// ✅ 好的做法
let data: MyData = match serde_json::from_str(&input) {
    Ok(d) => d,
    Err(e) => {
        error!("Failed to parse JSON: {}", e);
        return 0;
    }
};
```

### 2. 日志使用

合理使用日志级别。

```rust
trace!("Entering function with params: {:?}", params);  // 详细追踪
debug!("Processing {} items", items.len());             // 调试信息
info!("Successfully processed message");                // 正常信息
warn!("Unusual value detected: {}", value);             // 警告
error!("Critical failure: {}", error);                  // 错误
```

### 3. 内存管理

避免大量内存分配，使用栈上分配。

```rust
// ❌ 避免大量堆分配
let mut vec = Vec::with_capacity(1000000);

// ✅ 使用合理的容量
let mut vec = Vec::with_capacity(100);
```

### 4. 性能优化

- 使用 `serde_json::from_str` 而不是 `serde_json::from_slice`
- 避免不必要的克隆
- 使用 `&str` 而不是 `String`（当不需要所有权时）

```rust
// ✅ 高效的字符串处理
fn process_message(msg: &str) -> Result<(), String> {
    // 不需要克隆
    let parts: Vec<&str> = msg.split(',').collect();
    Ok(())
}
```

### 5. 体积优化

在 `Cargo.toml` 中配置优化选项：

```toml
[profile.release]
opt-level = "z"     # 优化体积
lto = true          # 链接时优化
strip = true        # 移除符号表
panic = "abort"     # 使用 abort 而不是 unwind
codegen-units = 1   # 单个代码生成单元
```

---

## 调试技巧

### 1. 使用日志

在关键位置添加日志：

```rust
info!("Input: {}", input);
debug!("Parsed data: {:?}", data);
trace!("Processing step 1 complete");
```

### 2. 单元测试

为插件逻辑编写单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_data() {
        let input = r#"{"temperature": 25.5}"#;
        let data: SensorData = serde_json::from_str(input).unwrap();
        assert_eq!(data.temperature, 25.5);
    }
}
```

### 3. 本地测试

在编译为 Wasm 之前，先在本地测试逻辑：

```rust
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let input = r#"{"device_id": "test", "temperature": 25.5}"#;
    // 测试你的逻辑
}
```

### 4. 查看 Wasm 文件大小

```bash
ls -lh target/wasm32-unknown-unknown/release/*.wasm
```

### 5. 优化后对比

```bash
# 优化前
wasm-opt -Oz input.wasm -o output.wasm

# 查看优化效果
ls -lh input.wasm output.wasm
```

---

## 常见问题

### Q1: 插件无法加载

**问题**: `Failed to load plugin: invalid wasm module`

**解决方案**:
- 确保使用 `wasm32-unknown-unknown` 目标编译
- 检查 `Cargo.toml` 中 `crate-type = ["cdylib"]`
- 确保导出了 `alloc` 和 `dealloc` 函数

### Q2: 内存访问错误

**问题**: `Invalid memory range in plugin`

**解决方案**:
- 检查 `ptr` 和 `len` 参数是否正确
- 确保使用 `read_string_from_host` 读取数据
- 不要手动操作内存指针

### Q3: 日志不显示

**问题**: 插件日志没有输出

**解决方案**:
```bash
# 设置正确的日志级别
export RUST_LOG=wasm_plugin=debug
cargo run -p flux-server
```

### Q4: 编译体积过大

**问题**: Wasm 文件超过 1MB

**解决方案**:
- 使用 `opt-level = "z"`
- 启用 `lto = true`
- 移除不必要的依赖
- 使用 `wasm-opt` 优化

### Q5: 函数找不到

**问题**: `Plugin must export 'on_msg' function`

**解决方案**:
- 确保函数使用 `#[no_mangle]`
- 确保函数签名正确: `pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32`
- 检查函数名拼写

---

## 参考资料

- [Rust Wasm Book](https://rustwasm.github.io/docs/book/)
- [Wasmtime 文档](https://docs.wasmtime.dev/)
- [Serde 文档](https://serde.rs/)
- [FLUX IOT 插件集成指南](plugin_integration_guide.md)

---

## 下一步

- 查看 [API 文档](API.md) 了解如何与平台交互
- 查看 [部署指南](DEPLOYMENT.md) 了解如何部署插件
- 参考 `plugins/dummy_plugin` 示例代码

---

**Happy Coding! 🚀**
