# 阶段 6：规则引擎 - 设计方案

> **设计日期**: 2026-02-22  
> **版本**: v1.0.0  
> **状态**: 📋 **方案设计中**

---

## 🎯 规则引擎概述

### 什么是规则引擎？

规则引擎是一个**基于条件-动作模式**的自动化系统，用于：
- 监控设备状态和数据
- 根据预定义规则自动触发动作
- 实现复杂的业务逻辑
- 提供灵活的配置化能力

### 核心价值

**1. 自动化决策** 🤖
- 无需人工干预
- 实时响应
- 降低运营成本

**2. 灵活配置** 🔧
- 可视化规则配置
- 动态规则更新
- 无需修改代码

**3. 业务价值** 💰
- 提升效率
- 降低错误
- 增强用户体验

---

## 📊 应用场景分析

### 场景 1: 设备告警

**需求**:
```
当温度 > 80°C 时，发送告警通知
当压力 < 10 PSI 时，关闭阀门
当设备离线超过 5 分钟，发送短信通知
```

**规则示例**:
```yaml
rule:
  name: "高温告警"
  condition: "device.temperature > 80"
  actions:
    - type: "notification"
      channel: "email"
      message: "设备温度过高: {{device.temperature}}°C"
```

---

### 场景 2: 设备联动

**需求**:
```
当门禁打开时，自动开启走廊灯光
当检测到烟雾时，关闭空调，开启排风扇
当会议室有人时，自动调节温度到 24°C
```

**规则示例**:
```yaml
rule:
  name: "门禁联动照明"
  condition: "door.status == 'open'"
  actions:
    - type: "device_control"
      device: "corridor_light"
      command: "turn_on"
```

---

### 场景 3: 数据处理

**需求**:
```
当连续 3 次读数异常时，标记设备故障
当日均能耗超过阈值时，生成报告
当数据缺失时，使用上一次有效值填充
```

**规则示例**:
```yaml
rule:
  name: "异常检测"
  condition: "count(device.errors, 5min) >= 3"
  actions:
    - type: "update_status"
      status: "fault"
    - type: "create_ticket"
      priority: "high"
```

---

### 场景 4: 定时任务

**需求**:
```
每天 8:00 开启空调
每周一生成能耗报告
每月 1 号清理历史数据
```

**规则示例**:
```yaml
rule:
  name: "定时开启空调"
  trigger: "cron(0 8 * * *)"
  actions:
    - type: "device_control"
      device: "air_conditioner"
      command: "turn_on"
      params:
        temperature: 24
```

---

## 🏗️ 架构设计

### 方案对比

#### 方案 A: 基于 Rhai 脚本引擎 ⭐ **推荐**

**优势**:
- ✅ 已集成 Rhai（flux-script 包）
- ✅ 安全沙箱环境
- ✅ 高性能（编译缓存）
- ✅ 灵活的脚本语法
- ✅ 易于扩展

**劣势**:
- ⚠️ 需要学习 Rhai 语法
- ⚠️ 调试相对复杂

**示例**:
```rust
// Rhai 规则脚本
if device.temperature > 80 {
    send_notification("高温告警", device.temperature);
    control_device("fan", "turn_on");
}
```

---

#### 方案 B: 基于 JSON 规则配置

**优势**:
- ✅ 简单易懂
- ✅ 易于序列化
- ✅ 可视化配置友好

**劣势**:
- ❌ 表达能力有限
- ❌ 复杂逻辑难以实现
- ❌ 需要自己实现解析器

**示例**:
```json
{
  "condition": {
    "operator": "AND",
    "conditions": [
      {"field": "temperature", "operator": ">", "value": 80},
      {"field": "humidity", "operator": "<", "value": 30}
    ]
  },
  "actions": [
    {"type": "notification", "message": "告警"}
  ]
}
```

---

#### 方案 C: 基于 Rete 算法

**优势**:
- ✅ 高性能（适合大量规则）
- ✅ 模式匹配强大

**劣势**:
- ❌ 实现复杂
- ❌ Rust 生态支持少
- ❌ 学习曲线陡峭

---

### 推荐方案：Rhai + JSON 混合

**核心思路**:
1. **简单规则**: 使用 JSON 配置（易于可视化）
2. **复杂规则**: 使用 Rhai 脚本（灵活强大）
3. **统一引擎**: 底层都转换为 Rhai 执行

**优势**:
- ✅ 兼顾易用性和灵活性
- ✅ 渐进式学习曲线
- ✅ 复用现有 flux-script
- ✅ 统一执行引擎

---

## 🔧 技术架构

### 整体架构

```
┌─────────────────────────────────────────┐
│         规则配置层 (UI/API)              │
│  JSON 规则 / Rhai 脚本 / 可视化编辑器    │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│         规则管理层 (flux-rule)           │
│  规则解析 / 规则验证 / 规则存储          │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│         规则引擎层 (RuleEngine)          │
│  条件评估 / 动作执行 / 上下文管理        │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│         执行层 (Rhai Engine)             │
│  脚本编译 / 脚本执行 / 函数注册          │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│         设备层 (Device/Protocol)         │
│  设备控制 / 数据读取 / 状态监控          │
└─────────────────────────────────────────┘
```

---

### 核心模块

#### 1. 规则模型 (Rule Model)

```rust
pub struct Rule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    
    // 触发器
    pub trigger: RuleTrigger,
    
    // 条件
    pub condition: RuleCondition,
    
    // 动作
    pub actions: Vec<RuleAction>,
    
    // 优先级
    pub priority: i32,
    
    // 元数据
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum RuleTrigger {
    // 设备事件触发
    DeviceEvent {
        device_id: String,
        event_type: String,
    },
    
    // 数据变化触发
    DataChange {
        metric: String,
        threshold: f64,
    },
    
    // 定时触发
    Schedule {
        cron: String,
    },
    
    // 手动触发
    Manual,
}

pub enum RuleCondition {
    // 简单条件（JSON）
    Simple {
        field: String,
        operator: ComparisonOperator,
        value: Value,
    },
    
    // 复合条件
    Composite {
        operator: LogicalOperator,
        conditions: Vec<RuleCondition>,
    },
    
    // 脚本条件（Rhai）
    Script {
        script: String,
    },
}

pub enum RuleAction {
    // 设备控制
    DeviceControl {
        device_id: String,
        command: String,
        params: HashMap<String, Value>,
    },
    
    // 发送通知
    Notification {
        channel: String,
        message: String,
    },
    
    // 数据写入
    DataWrite {
        target: String,
        value: Value,
    },
    
    // 执行脚本
    Script {
        script: String,
    },
    
    // HTTP 请求
    HttpRequest {
        url: String,
        method: String,
        body: Option<Value>,
    },
}
```

---

#### 2. 规则引擎 (Rule Engine)

```rust
pub struct RuleEngine {
    // Rhai 脚本引擎
    script_engine: Arc<ScriptEngine>,
    
    // 规则存储
    rules: Arc<RwLock<HashMap<String, Rule>>>,
    
    // 上下文管理
    context: Arc<RwLock<RuleContext>>,
    
    // 动作执行器
    action_executor: Arc<ActionExecutor>,
}

impl RuleEngine {
    /// 评估规则
    pub async fn evaluate_rule(&self, rule: &Rule, context: &RuleContext) -> Result<bool> {
        match &rule.condition {
            RuleCondition::Simple { field, operator, value } => {
                self.evaluate_simple_condition(field, operator, value, context).await
            }
            RuleCondition::Composite { operator, conditions } => {
                self.evaluate_composite_condition(operator, conditions, context).await
            }
            RuleCondition::Script { script } => {
                self.evaluate_script_condition(script, context).await
            }
        }
    }
    
    /// 执行动作
    pub async fn execute_actions(&self, actions: &[RuleAction], context: &RuleContext) -> Result<()> {
        for action in actions {
            self.action_executor.execute(action, context).await?;
        }
        Ok(())
    }
    
    /// 触发规则
    pub async fn trigger_rule(&self, rule_id: &str, context: RuleContext) -> Result<()> {
        let rules = self.rules.read().await;
        let rule = rules.get(rule_id).ok_or("Rule not found")?;
        
        if !rule.enabled {
            return Ok(());
        }
        
        // 评估条件
        if self.evaluate_rule(rule, &context).await? {
            // 执行动作
            self.execute_actions(&rule.actions, &context).await?;
        }
        
        Ok(())
    }
}
```

---

#### 3. 规则上下文 (Rule Context)

```rust
pub struct RuleContext {
    // 设备数据
    pub device_data: HashMap<String, Value>,
    
    // 系统变量
    pub system_vars: HashMap<String, Value>,
    
    // 时间信息
    pub timestamp: DateTime<Utc>,
    
    // 触发事件
    pub trigger_event: Option<TriggerEvent>,
}

impl RuleContext {
    /// 获取字段值
    pub fn get_field(&self, field: &str) -> Option<&Value> {
        // 支持点号路径: device.temperature
        let parts: Vec<&str> = field.split('.').collect();
        
        match parts[0] {
            "device" => self.device_data.get(parts.get(1)?),
            "system" => self.system_vars.get(parts.get(1)?),
            _ => None,
        }
    }
    
    /// 设置字段值
    pub fn set_field(&mut self, field: &str, value: Value) {
        // 实现字段设置逻辑
    }
}
```

---

## 💡 实现示例

### 示例 1: 简单 JSON 规则

```json
{
  "id": "rule_001",
  "name": "高温告警",
  "enabled": true,
  "trigger": {
    "type": "data_change",
    "metric": "temperature"
  },
  "condition": {
    "type": "simple",
    "field": "device.temperature",
    "operator": ">",
    "value": 80
  },
  "actions": [
    {
      "type": "notification",
      "channel": "email",
      "message": "设备温度过高: {{device.temperature}}°C"
    },
    {
      "type": "device_control",
      "device_id": "fan_001",
      "command": "turn_on"
    }
  ]
}
```

---

### 示例 2: 复杂 Rhai 规则

```rust
// Rhai 脚本规则
let temp = device.temperature;
let humidity = device.humidity;

// 复杂条件判断
if temp > 80 && humidity < 30 {
    // 发送告警
    send_notification("critical", `温度: ${temp}°C, 湿度: ${humidity}%`);
    
    // 控制设备
    control_device("fan", "turn_on", #{speed: "high"});
    control_device("humidifier", "turn_on");
    
    // 记录日志
    log_event("high_temp_low_humidity", #{temp: temp, humidity: humidity});
}

// 连续监控
if count_events("temp_high", "5min") >= 3 {
    send_notification("urgent", "连续高温告警");
    create_ticket("设备异常", "high");
}
```

---

### 示例 3: 定时规则

```yaml
rule:
  name: "每日报告"
  trigger:
    type: "schedule"
    cron: "0 8 * * *"  # 每天 8:00
  
  condition:
    type: "script"
    script: |
      // 检查是否为工作日
      let day = now().weekday();
      day >= 1 && day <= 5
  
  actions:
    - type: "script"
      script: |
        // 生成报告
        let report = generate_daily_report();
        send_email("admin@example.com", "日报", report);
```

---

## 📋 实施计划

### 第 1 天：规则模型和存储

**任务**:
- ✅ 定义规则数据模型
- ✅ 实现规则序列化/反序列化
- ✅ 实现规则存储（内存 + 数据库）

**代码量**: ~400 行

---

### 第 2-3 天：规则引擎核心

**任务**:
- ✅ 实现条件评估器
- ✅ 实现动作执行器
- ✅ 集成 Rhai 脚本引擎
- ✅ 实现上下文管理

**代码量**: ~800 行

---

### 第 4 天：触发器系统

**任务**:
- ✅ 实现事件触发器
- ✅ 实现定时触发器
- ✅ 实现数据变化监听

**代码量**: ~400 行

---

### 第 5 天：API 和集成

**任务**:
- ✅ 实现规则管理 API
- ✅ 集成设备控制
- ✅ 集成通知系统

**代码量**: ~400 行

---

### 第 6 天：测试和文档

**任务**:
- ✅ 单元测试
- ✅ 集成测试
- ✅ 示例程序
- ✅ 文档编写

**代码量**: ~200 行

---

## ✅ 总结

### 推荐方案

**Rhai + JSON 混合规则引擎**

**优势**:
- ✅ 复用现有 flux-script
- ✅ 简单规则用 JSON（易用）
- ✅ 复杂规则用 Rhai（强大）
- ✅ 统一执行引擎
- ✅ 安全沙箱环境

### 预期成果

**代码量**: ~2,200 行  
**工期**: 6 天（约 1 周）  
**完成度**: 100%

### 核心功能

- ✅ 规则定义和管理
- ✅ 条件评估
- ✅ 动作执行
- ✅ 多种触发器
- ✅ Rhai 脚本支持
- ✅ JSON 配置支持

---

**维护者**: FLUX IOT Team  
**设计日期**: 2026-02-22  
**状态**: 📋 **方案设计完成，待讨论确认**
