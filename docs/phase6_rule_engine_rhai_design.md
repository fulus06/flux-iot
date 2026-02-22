# 阶段 6：规则引擎 - 纯 Rhai 方案

> **设计日期**: 2026-02-22  
> **版本**: v2.0.0 (纯 Rhai)  
> **状态**: ✅ **方案确定**

---

## 🎯 设计决策

### 为什么选择纯 Rhai？

**1. 统一性** ✅
- 只需要学习一种语法（Rhai）
- 不需要 JSON → Rhai 转换
- 代码更简洁

**2. 强大性** ✅
- Rhai 可以表达任何复杂逻辑
- 支持函数、循环、条件
- 支持自定义函数注册

**3. 简化性** ✅
- 减少抽象层
- 减少代码量
- 更易维护

**4. 复用性** ✅
- 完全复用 `flux-script` 包
- 统一的脚本引擎
- 统一的沙箱环境

---

## 🏗️ 架构设计

### 整体架构（简化版）

```
┌─────────────────────────────────────────┐
│         规则配置层 (UI/API)              │
│         Rhai 脚本编辑器                  │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│         规则管理层 (flux-rule)           │
│  规则存储 / 规则加载 / 规则验证          │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│         规则引擎层 (RuleEngine)          │
│  触发器管理 / 上下文管理 / 规则执行      │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│         执行层 (flux-script)             │
│  Rhai 引擎 / 函数注册 / 沙箱执行         │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│         设备层 (Device/Protocol)         │
│  设备控制 / 数据读取 / 通知发送          │
└─────────────────────────────────────────┘
```

---

## 📊 核心数据模型

### 规则模型（简化）

```rust
/// 规则定义
pub struct Rule {
    /// 规则 ID
    pub id: String,
    
    /// 规则名称
    pub name: String,
    
    /// 规则描述
    pub description: String,
    
    /// 是否启用
    pub enabled: bool,
    
    /// 触发器类型
    pub trigger: RuleTrigger,
    
    /// Rhai 脚本（包含条件判断和动作执行）
    pub script: String,
    
    /// 优先级（1-100，数字越大优先级越高）
    pub priority: i32,
    
    /// 元数据
    pub metadata: RuleMetadata,
}

/// 触发器类型
pub enum RuleTrigger {
    /// 设备事件触发
    DeviceEvent {
        device_id: String,
        event_type: String,
    },
    
    /// 数据变化触发（任何数据更新都触发）
    DataChange {
        device_id: String,
        metric: Option<String>,  // None 表示任何指标
    },
    
    /// 定时触发
    Schedule {
        cron: String,
    },
    
    /// 手动触发
    Manual,
}

/// 规则元数据
pub struct RuleMetadata {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub tags: Vec<String>,
}
```

---

## 💡 Rhai 规则示例

### 示例 1: 简单告警

```rust
// 规则: 高温告警
// 触发器: DataChange { device_id: "sensor_001", metric: "temperature" }

// 获取设备数据
let temp = device.temperature;

// 条件判断
if temp > 80.0 {
    // 发送通知
    send_notification("email", "高温告警", `设备温度: ${temp}°C`);
    
    // 控制设备
    control_device("fan_001", "turn_on", #{});
    
    // 记录日志
    log("warn", `Temperature too high: ${temp}°C`);
}
```

---

### 示例 2: 设备联动

```rust
// 规则: 门禁联动照明
// 触发器: DeviceEvent { device_id: "door_001", event_type: "status_change" }

// 获取门禁状态
let door_status = device.status;

if door_status == "open" {
    // 开启走廊灯光
    control_device("light_corridor", "turn_on", #{
        brightness: 100,
        duration: 300  // 5分钟后自动关闭
    });
    
    // 记录进出日志
    log("info", `Door opened at ${now()}`);
    
    // 发送通知给管理员
    send_notification("push", "门禁开启", `门禁在 ${now()} 被打开`);
}
```

---

### 示例 3: 复杂业务逻辑

```rust
// 规则: 智能空调控制
// 触发器: DataChange { device_id: "sensor_room", metric: null }

// 获取传感器数据
let temp = device.temperature;
let humidity = device.humidity;
let occupancy = device.occupancy;

// 复杂条件判断
if occupancy {
    // 有人时的逻辑
    if temp > 26.0 {
        control_device("ac_001", "set_temperature", #{
            temperature: 24,
            mode: "cool"
        });
    } else if temp < 20.0 {
        control_device("ac_001", "set_temperature", #{
            temperature: 22,
            mode: "heat"
        });
    }
    
    // 湿度控制
    if humidity > 70.0 {
        control_device("dehumidifier_001", "turn_on", #{});
    } else if humidity < 30.0 {
        control_device("humidifier_001", "turn_on", #{});
    }
} else {
    // 无人时关闭空调（节能）
    control_device("ac_001", "turn_off", #{});
}
```

---

### 示例 4: 异常检测

```rust
// 规则: 连续异常检测
// 触发器: DataChange { device_id: "machine_001", metric: "vibration" }

let vibration = device.vibration;

// 检查是否超过阈值
if vibration > 5.0 {
    // 记录异常事件
    record_event("high_vibration", #{
        device_id: "machine_001",
        value: vibration,
        timestamp: now()
    });
    
    // 检查最近 5 分钟内的异常次数
    let count = count_events("high_vibration", "5min");
    
    if count >= 3 {
        // 连续 3 次异常，触发告警
        send_notification("urgent", "设备异常", 
            `设备振动异常，最近5分钟内发生 ${count} 次`);
        
        // 创建工单
        create_ticket(#{
            title: "设备振动异常",
            device_id: "machine_001",
            priority: "high",
            description: `振动值: ${vibration}, 次数: ${count}`
        });
        
        // 标记设备状态
        update_device_status("machine_001", "fault");
    }
}
```

---

### 示例 5: 定时任务

```rust
// 规则: 每日能耗报告
// 触发器: Schedule { cron: "0 8 * * *" }

// 获取昨天的日期
let yesterday = date_add(now(), -1, "day");

// 查询能耗数据
let energy_data = query_metrics(#{
    metric: "energy_consumption",
    start_time: date_start_of_day(yesterday),
    end_time: date_end_of_day(yesterday),
    aggregation: "sum"
});

// 生成报告
let report = `
能耗日报 - ${format_date(yesterday, "YYYY-MM-DD")}

总能耗: ${energy_data.total} kWh
平均功率: ${energy_data.average} kW
峰值功率: ${energy_data.peak} kW

详细数据请查看附件。
`;

// 发送邮件
send_email(#{
    to: "admin@example.com",
    subject: "能耗日报",
    body: report,
    attachments: [
        generate_csv(energy_data)
    ]
});
```

---

## 🔧 Rhai 内置函数

### 设备控制函数

```rust
// 控制设备
control_device(device_id, command, params)

// 读取设备数据
read_device(device_id, metric)

// 更新设备状态
update_device_status(device_id, status)
```

---

### 通知函数

```rust
// 发送通知
send_notification(channel, title, message)

// 发送邮件
send_email(params)

// 发送短信
send_sms(phone, message)

// 发送推送
send_push(user_id, title, message)
```

---

### 数据查询函数

```rust
// 查询指标数据
query_metrics(params)

// 统计事件次数
count_events(event_type, time_range)

// 记录事件
record_event(event_type, data)
```

---

### 工单函数

```rust
// 创建工单
create_ticket(params)

// 更新工单
update_ticket(ticket_id, params)

// 关闭工单
close_ticket(ticket_id)
```

---

### 时间函数

```rust
// 当前时间
now()

// 日期加减
date_add(date, amount, unit)

// 格式化日期
format_date(date, format)

// 获取日期开始/结束
date_start_of_day(date)
date_end_of_day(date)
```

---

### 日志函数

```rust
// 记录日志
log(level, message)

// 调试日志
debug(message)

// 信息日志
info(message)

// 警告日志
warn(message)

// 错误日志
error(message)
```

---

## 📋 实施计划

### 第 1 天：规则模型和存储

**任务**:
- ✅ 定义 `Rule` 数据结构
- ✅ 实现规则序列化/反序列化
- ✅ 实现规则存储（数据库）
- ✅ 实现规则 CRUD API

**代码量**: ~300 行

**文件**:
```
crates/flux-rule/
├── src/
│   ├── lib.rs
│   ├── model.rs      # 规则模型
│   └── storage.rs    # 规则存储
└── Cargo.toml
```

---

### 第 2-3 天：规则引擎核心

**任务**:
- ✅ 实现 `RuleEngine` 核心
- ✅ 集成 `flux-script` (Rhai)
- ✅ 实现上下文管理
- ✅ 注册内置函数

**代码量**: ~600 行

**文件**:
```
crates/flux-rule/src/
├── engine.rs         # 规则引擎
├── context.rs        # 执行上下文
└── functions.rs      # 内置函数注册
```

---

### 第 4 天：触发器系统

**任务**:
- ✅ 实现事件触发器
- ✅ 实现定时触发器（Cron）
- ✅ 实现数据变化监听
- ✅ 触发器调度

**代码量**: ~400 行

**文件**:
```
crates/flux-rule/src/
├── trigger.rs        # 触发器
└── scheduler.rs      # 调度器
```

---

### 第 5 天：集成和测试

**任务**:
- ✅ 集成设备控制
- ✅ 集成通知系统
- ✅ 单元测试
- ✅ 集成测试

**代码量**: ~300 行

---

### 第 6 天：示例和文档

**任务**:
- ✅ 编写示例规则
- ✅ 编写 README
- ✅ 编写 API 文档
- ✅ 编写最佳实践

**代码量**: ~200 行

---

## 🎯 核心实现

### RuleEngine 核心代码

```rust
use flux_script::ScriptEngine;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RuleEngine {
    // Rhai 脚本引擎
    script_engine: Arc<ScriptEngine>,
    
    // 规则存储
    rules: Arc<RwLock<HashMap<String, Rule>>>,
    
    // 触发器调度器
    scheduler: Arc<TriggerScheduler>,
}

impl RuleEngine {
    pub fn new() -> Self {
        let mut script_engine = ScriptEngine::new();
        
        // 注册内置函数
        register_device_functions(&mut script_engine);
        register_notification_functions(&mut script_engine);
        register_data_functions(&mut script_engine);
        register_time_functions(&mut script_engine);
        
        Self {
            script_engine: Arc::new(script_engine),
            rules: Arc::new(RwLock::new(HashMap::new())),
            scheduler: Arc::new(TriggerScheduler::new()),
        }
    }
    
    /// 执行规则
    pub async fn execute_rule(&self, rule_id: &str, context: RuleContext) -> Result<()> {
        let rules = self.rules.read().await;
        let rule = rules.get(rule_id).ok_or("Rule not found")?;
        
        if !rule.enabled {
            return Ok(());
        }
        
        // 准备脚本上下文
        let mut scope = rhai::Scope::new();
        
        // 注入设备数据
        scope.push("device", context.device_data);
        
        // 注入系统变量
        scope.push("system", context.system_vars);
        
        // 执行 Rhai 脚本
        self.script_engine.eval_with_scope(&mut scope, &rule.script)?;
        
        Ok(())
    }
    
    /// 添加规则
    pub async fn add_rule(&self, rule: Rule) -> Result<()> {
        // 验证脚本语法
        self.script_engine.compile(&rule.script)?;
        
        // 存储规则
        let mut rules = self.rules.write().await;
        rules.insert(rule.id.clone(), rule.clone());
        
        // 注册触发器
        self.scheduler.register_trigger(&rule).await?;
        
        Ok(())
    }
}
```

---

## ✅ 优势总结

### 技术优势

**1. 简洁性** ✅
- 只需要 Rhai 一种语法
- 减少抽象层
- 代码量减少 ~30%

**2. 强大性** ✅
- 完整的编程能力
- 支持复杂逻辑
- 支持自定义函数

**3. 统一性** ✅
- 与 flux-script 完全统一
- 统一的沙箱环境
- 统一的错误处理

**4. 性能** ✅
- Rhai 编译缓存
- 零开销抽象
- 高效执行

---

### 开发优势

**1. 学习成本低** ✅
- 只需学习 Rhai（类 Rust 语法）
- 不需要学习 JSON 规则格式
- 文档统一

**2. 调试方便** ✅
- 直接查看 Rhai 脚本
- 清晰的错误信息
- 支持测试模式

**3. 可维护性高** ✅
- 代码即文档
- 易于理解
- 易于修改

---

## 📚 预期成果

### 代码量

| 模块 | 代码量 |
|------|--------|
| 规则模型和存储 | ~300 行 |
| 规则引擎核心 | ~600 行 |
| 触发器系统 | ~400 行 |
| 集成和测试 | ~300 行 |
| 示例和文档 | ~200 行 |
| **总计** | **~1,800 行** |

**比混合方案减少**: ~400 行（-18%）

---

### 功能清单

- ✅ 纯 Rhai 规则脚本
- ✅ 多种触发器（事件/定时/数据变化）
- ✅ 丰富的内置函数
- ✅ 规则管理 API
- ✅ 规则优先级
- ✅ 规则测试模式
- ✅ 完整文档和示例

---

## 🚀 下一步

**方案已确定**: 纯 Rhai 规则引擎

**预计工期**: 6 天（约 1 周）

**准备开始实施？**

---

**维护者**: FLUX IOT Team  
**设计日期**: 2026-02-22  
**状态**: ✅ **方案确定，准备实施**
