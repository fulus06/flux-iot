# 场景联动设计方案 - Rhai 集成分析

> **分析日期**: 2026-02-22  
> **结论**: ✅ **推荐使用 Rhai**

---

## 🎯 核心问题

**场景联动是否需要 Rhai 动态脚本引擎？**

**答案**: ✅ **是的，强烈推荐**

---

## 📊 方案对比

### 方案 A：纯 Rust 硬编码 ❌

**实现方式**:
```rust
pub struct Scene {
    pub triggers: Vec<Trigger>,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
}

pub enum Condition {
    DeviceStatus { device_id: String, status: String },
    MetricThreshold { device_id: String, metric: String, operator: Operator, value: f64 },
    TimeRange { start: String, end: String },
}
```

**优点**:
- ✅ 类型安全
- ✅ 性能最优
- ✅ 编译时检查

**缺点**:
- ❌ **灵活性差** - 每种条件都需要硬编码
- ❌ **扩展困难** - 添加新逻辑需要修改代码重新编译
- ❌ **用户无法自定义** - 只能使用预定义的条件和动作
- ❌ **复杂逻辑难以表达** - 嵌套条件、循环等需要大量枚举

---

### 方案 B：使用 Rhai 脚本引擎 ✅ **推荐**

**实现方式**:
```rust
pub struct Scene {
    pub name: String,
    pub triggers: Vec<Trigger>,
    pub condition_script: Option<String>,  // Rhai 脚本
    pub action_script: String,              // Rhai 脚本
}

// 条件脚本示例
let condition_script = r#"
    let temp = get_device_metric("sensor_01", "temperature");
    let humidity = get_device_metric("sensor_02", "humidity");
    
    temp > 30.0 && humidity < 40.0
"#;

// 动作脚本示例
let action_script = r#"
    send_command("fan_01", "set_state", #{state: true});
    send_command("humidifier_01", "set_state", #{state: true});
    
    if get_device_status("window_01") == "closed" {
        send_notification("温度过高，已开启风扇和加湿器");
    }
"#;
```

**优点**:
- ✅ **极高灵活性** - 用户可以编写任意逻辑
- ✅ **动态更新** - 无需重启服务即可修改场景
- ✅ **易于扩展** - 通过注册函数即可添加新功能
- ✅ **复杂逻辑支持** - 支持条件、循环、函数等
- ✅ **安全沙箱** - Rhai 提供安全的执行环境
- ✅ **已有基础** - 项目中已有 `flux-script` 包

**缺点**:
- ⚠️ 运行时开销（但可接受）
- ⚠️ 需要脚本调试工具

---

## 💡 为什么推荐 Rhai？

### 1. 项目已有 Rhai 基础 ✅

项目中已经有 `flux-script` 包，说明：
- Rhai 已经集成到项目中
- 团队熟悉 Rhai
- 可以复用现有代码和经验

### 2. 场景联动的核心需求

**需求分析**:

| 需求 | 硬编码方案 | Rhai 方案 |
|------|-----------|----------|
| **复杂条件判断** | ❌ 枚举爆炸 | ✅ 脚本灵活 |
| **动态更新场景** | ❌ 需重启 | ✅ 热更新 |
| **用户自定义逻辑** | ❌ 不支持 | ✅ 完全支持 |
| **嵌套条件** | ❌ 难实现 | ✅ 原生支持 |
| **循环和迭代** | ❌ 不支持 | ✅ 支持 |
| **时间计算** | ⚠️ 有限 | ✅ 灵活 |

### 3. 实际场景示例

#### 场景 1：温度控制（简单）

**硬编码方式**:
```rust
// 需要预定义所有可能的条件组合
if device.metric("temperature") > 30.0 {
    send_command("fan", "on");
}
```

**Rhai 方式**:
```rust
// 用户可以自由编写逻辑
let temp = get_metric("sensor_01", "temperature");
if temp > 30.0 {
    send_command("fan_01", "set_state", #{state: true});
}
```

#### 场景 2：智能灌溉（复杂）

**硬编码方式**:
```rust
// 需要为每种情况创建枚举
enum IrrigationCondition {
    SoilMoistureLow { threshold: f64 },
    WeatherSunny,
    TimeInRange { start: Time, end: Time },
    Combined { conditions: Vec<IrrigationCondition> },
}
// 维护成本极高！
```

**Rhai 方式**:
```rust
// 用户可以灵活组合逻辑
let moisture = get_metric("soil_sensor", "moisture");
let weather = get_weather();
let hour = get_hour();

if moisture < 30.0 && weather == "sunny" && hour >= 6 && hour <= 8 {
    // 早上6-8点，土壤湿度低，天气晴朗，开始灌溉
    send_command("irrigation_01", "start", #{duration: 1800});
    
    // 30分钟后检查
    schedule_check(1800, || {
        let new_moisture = get_metric("soil_sensor", "moisture");
        if new_moisture < 50.0 {
            send_notification("灌溉效果不佳，请检查系统");
        }
    });
}
```

#### 场景 3：多设备联动（超复杂）

**Rhai 方式**:
```rust
// 办公室智能控制
let people_count = get_metric("people_counter", "count");
let light_level = get_metric("light_sensor", "lux");
let time = get_hour();

// 工作时间且有人
if time >= 9 && time <= 18 && people_count > 0 {
    // 根据光照调整灯光
    if light_level < 300 {
        let brightness = 100 - (light_level / 300.0 * 100.0);
        send_command("lights", "set_brightness", #{value: brightness});
    }
    
    // 根据人数调整空调
    let ac_temp = 26.0 - (people_count / 10.0);
    send_command("ac", "set_temperature", #{value: ac_temp});
    
    // 人多时开启新风系统
    if people_count > 20 {
        send_command("ventilation", "set_state", #{state: true});
    }
} else {
    // 下班后节能模式
    send_command("lights", "set_state", #{state: false});
    send_command("ac", "set_mode", #{mode: "eco"});
}
```

**硬编码方式**: 几乎不可能优雅实现！

---

## 🏗️ 推荐架构

### 场景模型

```rust
pub struct Scene {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    
    // 触发器（何时执行）
    pub triggers: Vec<Trigger>,
    
    // 条件脚本（是否执行）- Rhai
    pub condition_script: Option<String>,
    
    // 动作脚本（执行什么）- Rhai
    pub action_script: String,
    
    pub enabled: bool,
}

pub enum Trigger {
    Manual,                          // 手动触发
    Schedule(CronExpression),        // 定时触发
    DeviceEvent {                    // 设备事件触发
        device_id: String,
        event_type: String,
    },
    MetricChange {                   // 指标变化触发
        device_id: String,
        metric: String,
    },
}
```

### Rhai 引擎集成

```rust
use flux_script::ScriptEngine;

pub struct SceneEngine {
    script_engine: ScriptEngine,
    command_executor: Arc<CommandExecutor>,
    device_manager: Arc<DeviceManager>,
}

impl SceneEngine {
    pub fn new(...) -> Self {
        let mut engine = ScriptEngine::new();
        
        // 注册设备控制函数
        engine.register_fn("send_command", |device_id: &str, cmd: &str, params: Map| {
            // 发送指令到设备
        });
        
        // 注册设备查询函数
        engine.register_fn("get_device_status", |device_id: &str| -> String {
            // 查询设备状态
        });
        
        engine.register_fn("get_metric", |device_id: &str, metric: &str| -> f64 {
            // 查询设备指标
        });
        
        // 注册通知函数
        engine.register_fn("send_notification", |message: &str| {
            // 发送通知
        });
        
        // 注册时间函数
        engine.register_fn("get_hour", || -> i64 {
            chrono::Local::now().hour() as i64
        });
        
        Self { script_engine: engine, ... }
    }
    
    pub async fn execute_scene(&self, scene: &Scene) -> Result<()> {
        // 1. 检查条件
        if let Some(condition) = &scene.condition_script {
            let result: bool = self.script_engine.eval(condition)?;
            if !result {
                return Ok(()); // 条件不满足，不执行
            }
        }
        
        // 2. 执行动作
        self.script_engine.eval(&scene.action_script)?;
        
        Ok(())
    }
}
```

---

## 📋 实施计划

### 阶段 1：基础集成（1-2天）

1. **复用 flux-script**
   - 检查现有 Rhai 集成
   - 扩展必要的函数

2. **场景模型**
   - 定义 Scene 结构
   - 数据库表设计

3. **基础引擎**
   - SceneEngine 实现
   - 注册核心函数

### 阶段 2：功能完善（2-3天）

4. **触发器系统**
   - 定时触发（Cron）
   - 事件触发
   - 指标变化触发

5. **脚本函数库**
   - 设备控制函数
   - 设备查询函数
   - 时间函数
   - 通知函数

6. **场景管理 API**
   - 创建/更新/删除场景
   - 启用/禁用场景
   - 手动执行场景

### 阶段 3：高级特性（1-2天）

7. **脚本调试**
   - 语法检查
   - 执行日志
   - 错误处理

8. **性能优化**
   - 脚本缓存
   - 并发执行

---

## 🎯 推荐的 Rhai 函数库

### 设备控制

```rust
// 发送指令
send_command(device_id, command_type, params)

// 批量控制
send_batch_commands(device_ids, command_type, params)
```

### 设备查询

```rust
// 获取设备状态
get_device_status(device_id) -> String

// 获取设备指标
get_metric(device_id, metric_name) -> f64

// 获取设备信息
get_device_info(device_id) -> Map
```

### 时间函数

```rust
get_hour() -> i64
get_minute() -> i64
get_day_of_week() -> i64
is_weekend() -> bool
```

### 通知函数

```rust
send_notification(message)
send_email(to, subject, body)
send_sms(phone, message)
```

### 工具函数

```rust
log(message)
sleep(seconds)
schedule(delay_seconds, callback)
```

---

## 💰 成本收益分析

### 开发成本

| 方案 | 初期开发 | 维护成本 | 扩展成本 |
|------|---------|---------|---------|
| **硬编码** | 低 | 高 | 极高 |
| **Rhai** | 中 | 低 | 低 |

### 用户价值

| 方案 | 灵活性 | 可定制性 | 学习曲线 |
|------|-------|---------|---------|
| **硬编码** | 低 | 无 | 低 |
| **Rhai** | 极高 | 完全 | 中 |

---

## ✅ 最终建议

### 推荐方案：**Rhai 脚本引擎** ✅

**理由**:
1. ✅ 项目已有 `flux-script` 包，集成成本低
2. ✅ 场景联动需要极高的灵活性
3. ✅ 用户可以自定义复杂逻辑
4. ✅ 支持动态更新，无需重启
5. ✅ Rhai 安全、轻量、易用

### 混合方案（最佳实践）

```rust
pub struct Scene {
    // 简单场景：使用预定义模板
    pub template: Option<SceneTemplate>,
    
    // 复杂场景：使用 Rhai 脚本
    pub condition_script: Option<String>,
    pub action_script: Option<String>,
}

pub enum SceneTemplate {
    TemperatureControl { threshold: f64, action: Action },
    TimeSchedule { cron: String, action: Action },
    // ... 其他常用模板
}
```

**优点**:
- 简单场景使用模板（快速、易用）
- 复杂场景使用脚本（灵活、强大）
- 两者可以共存

---

## 📚 参考资料

- Rhai 官方文档: https://rhai.rs/
- flux-script 包: `crates/flux-script/`
- 场景联动最佳实践: Home Assistant, Node-RED

---

**结论**: ✅ **强烈推荐使用 Rhai 实现场景联动**

**下一步**: 基于 `flux-script` 实现 SceneEngine

---

**分析人员**: FLUX IOT Team  
**分析日期**: 2026-02-22  
**建议**: 🔥 **立即采用 Rhai 方案**
