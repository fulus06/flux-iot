# 规则引擎 vs 场景联动 - 概念辨析

> **日期**: 2026-02-22  
> **结论**: 场景联动是规则引擎的一个**子集/特例**

---

## 🎯 核心关系

```
┌─────────────────────────────────────┐
│         规则引擎 (Rule Engine)       │
│         ↓                            │
│    ┌─────────────────────┐          │
│    │   场景联动 (Scene)   │          │
│    │   (简化的规则)       │          │
│    └─────────────────────┘          │
│         ↓                            │
│    其他高级规则...                   │
└─────────────────────────────────────┘
```

**结论**: 场景联动 ⊂ 规则引擎

---

## 📊 概念对比

### 场景联动 (Scene Linkage)

**定义**: 预定义的设备联动场景

**特点**:
- ✅ 简单直观
- ✅ 一键触发
- ✅ 固定的条件-动作
- ✅ 用户友好

**示例**:
```yaml
场景: 回家模式
触发: 手动触发 / 定时触发
动作:
  - 开启客厅灯光
  - 打开空调
  - 播放音乐
```

---

### 规则引擎 (Rule Engine)

**定义**: 灵活的自动化决策系统

**特点**:
- ✅ 复杂逻辑
- ✅ 条件判断
- ✅ 数据处理
- ✅ 动态执行

**示例**:
```rust
// 智能温控规则
let temp = device.temperature;
let time = now().hour();
let occupancy = device.occupancy;

if occupancy {
    if time >= 18 && time <= 22 {
        // 晚上在家，舒适温度
        if temp > 26.0 {
            control_device("ac", "set_temperature", #{temp: 24});
        }
    } else {
        // 其他时间，节能温度
        if temp > 28.0 {
            control_device("ac", "set_temperature", #{temp: 26});
        }
    }
} else {
    // 无人时关闭
    control_device("ac", "turn_off", #{});
}
```

---

## 🔍 详细对比

| 维度 | 场景联动 | 规则引擎 |
|------|---------|---------|
| **复杂度** | 简单 | 复杂 |
| **条件判断** | 单一条件 | 多条件组合 |
| **逻辑能力** | 固定流程 | 编程逻辑 |
| **数据处理** | 不支持 | 支持 |
| **学习成本** | 低 | 中 |
| **灵活性** | 低 | 高 |
| **适用场景** | 日常场景 | 复杂业务 |

---

## 💡 实际应用

### 场景联动适用场景

**1. 日常生活场景**
```
- 回家模式: 开灯 + 开空调 + 开电视
- 离家模式: 关灯 + 关空调 + 启动安防
- 睡眠模式: 关灯 + 关窗帘 + 静音
- 观影模式: 关灯 + 拉窗帘 + 打开投影
```

**2. 简单定时任务**
```
- 每天 7:00 开启热水器
- 每天 22:00 关闭客厅灯
```

**3. 一键操作**
```
- 会议模式: 关闭通知 + 静音 + 投影
- 演示模式: 开灯 + 投影 + 音响
```

---

### 规则引擎适用场景

**1. 智能决策**
```rust
// 根据多个条件智能决策
if temp > 30 && humidity > 70 && occupancy {
    // 高温高湿有人 → 强力制冷
    control_device("ac", "set", #{mode: "cool", temp: 22, fan: "high"});
} else if temp > 26 && occupancy {
    // 温度适中有人 → 舒适模式
    control_device("ac", "set", #{mode: "cool", temp: 24, fan: "auto"});
}
```

**2. 异常检测**
```rust
// 连续异常检测
if count_events("high_temp", "5min") >= 3 {
    send_notification("urgent", "连续高温告警");
    create_ticket(#{priority: "high"});
}
```

**3. 数据分析**
```rust
// 能耗分析
let daily_energy = query_metrics(#{
    metric: "energy",
    range: "1day",
    aggregation: "sum"
});

if daily_energy > threshold {
    send_notification("warning", `能耗超标: ${daily_energy} kWh`);
}
```

**4. 复杂联动**
```rust
// 多设备协同
let door_open = device.door.status == "open";
let motion_detected = device.motion.detected;
let light_level = device.light_sensor.value;

if door_open && motion_detected && light_level < 100 {
    control_device("corridor_light", "turn_on", #{brightness: 80});
    
    // 5分钟后自动关闭
    schedule_action("5min", || {
        control_device("corridor_light", "turn_off", #{});
    });
}
```

---

## 🏗️ 统一架构设计

### 方案：规则引擎包含场景联动

```
┌─────────────────────────────────────────┐
│         flux-rule (规则引擎)             │
│                                          │
│  ┌────────────────────────────────┐    │
│  │  Scene (场景联动)               │    │
│  │  - 简化的规则定义               │    │
│  │  - 用户友好的 API               │    │
│  │  - 一键触发                     │    │
│  └────────────────────────────────┘    │
│                                          │
│  ┌────────────────────────────────┐    │
│  │  Rule (高级规则)                │    │
│  │  - Rhai 脚本                    │    │
│  │  - 复杂逻辑                     │    │
│  │  - 数据处理                     │    │
│  └────────────────────────────────┘    │
│                                          │
│         共享底层引擎                     │
└─────────────────────────────────────────┘
```

---

## 💻 代码设计

### 场景联动（简化 API）

```rust
/// 场景定义
pub struct Scene {
    pub id: String,
    pub name: String,
    pub icon: String,
    
    /// 触发方式
    pub trigger: SceneTrigger,
    
    /// 动作列表（简化）
    pub actions: Vec<SceneAction>,
}

pub enum SceneTrigger {
    Manual,                    // 手动触发
    Schedule { cron: String }, // 定时触发
}

pub struct SceneAction {
    pub device_id: String,
    pub command: String,
    pub params: HashMap<String, Value>,
}

// 示例
let scene = Scene {
    id: "scene_home".to_string(),
    name: "回家模式".to_string(),
    icon: "home".to_string(),
    trigger: SceneTrigger::Manual,
    actions: vec![
        SceneAction {
            device_id: "light_living_room".to_string(),
            command: "turn_on".to_string(),
            params: hashmap!{"brightness" => 80},
        },
        SceneAction {
            device_id: "ac_001".to_string(),
            command: "turn_on".to_string(),
            params: hashmap!{"temperature" => 24},
        },
    ],
};
```

---

### 高级规则（Rhai 脚本）

```rust
/// 规则定义
pub struct Rule {
    pub id: String,
    pub name: String,
    pub trigger: RuleTrigger,
    pub script: String,  // Rhai 脚本
}

// 示例
let rule = Rule {
    id: "rule_smart_ac".to_string(),
    name: "智能空调控制".to_string(),
    trigger: RuleTrigger::DataChange {
        device_id: "sensor_room".to_string(),
        metric: None,
    },
    script: r#"
        let temp = device.temperature;
        let occupancy = device.occupancy;
        
        if occupancy {
            if temp > 26.0 {
                control_device("ac_001", "set_temperature", #{
                    temperature: 24,
                    mode: "cool"
                });
            }
        } else {
            control_device("ac_001", "turn_off", #{});
        }
    "#.to_string(),
};
```

---

### 内部转换

```rust
impl Scene {
    /// 将场景转换为规则
    pub fn to_rule(&self) -> Rule {
        // 生成 Rhai 脚本
        let script = self.generate_script();
        
        Rule {
            id: self.id.clone(),
            name: self.name.clone(),
            trigger: match &self.trigger {
                SceneTrigger::Manual => RuleTrigger::Manual,
                SceneTrigger::Schedule { cron } => RuleTrigger::Schedule {
                    cron: cron.clone(),
                },
            },
            script,
            ..Default::default()
        }
    }
    
    fn generate_script(&self) -> String {
        let mut script = String::new();
        
        for action in &self.actions {
            script.push_str(&format!(
                r#"control_device("{}", "{}", #{});"#,
                action.device_id,
                action.command,
                self.format_params(&action.params)
            ));
            script.push('\n');
        }
        
        script
    }
}
```

---

## ✅ 最终建议

### 统一实现方案

**1. 只实现规则引擎** ✅

**理由**:
- 场景联动是规则引擎的简化形式
- 避免重复开发
- 统一维护

**2. 提供两层 API**

**场景 API（简化）**:
```rust
// 用户友好的场景 API
scene_manager.create_scene(Scene {
    name: "回家模式",
    actions: vec![
        SceneAction { device: "light", command: "turn_on" },
        SceneAction { device: "ac", command: "turn_on" },
    ],
});
```

**规则 API（高级）**:
```rust
// 高级用户使用 Rhai 脚本
rule_engine.create_rule(Rule {
    name: "智能温控",
    script: r#"
        if device.temperature > 26 {
            control_device("ac", "turn_on", #{temp: 24});
        }
    "#,
});
```

**3. 内部统一执行**

```
Scene → 转换为 Rule → RuleEngine 执行
Rule → 直接 → RuleEngine 执行
```

---

## 📋 实施建议

### 包结构

```
crates/flux-rule/
├── src/
│   ├── lib.rs
│   ├── rule.rs          # 规则定义
│   ├── scene.rs         # 场景定义（简化 API）
│   ├── engine.rs        # 统一执行引擎
│   ├── trigger.rs       # 触发器
│   └── functions.rs     # 内置函数
└── Cargo.toml
```

### API 设计

```rust
// 场景管理器（简化 API）
pub struct SceneManager {
    rule_engine: Arc<RuleEngine>,
}

impl SceneManager {
    pub async fn create_scene(&self, scene: Scene) -> Result<String> {
        // 转换为规则
        let rule = scene.to_rule();
        
        // 添加到规则引擎
        self.rule_engine.add_rule(rule).await
    }
    
    pub async fn trigger_scene(&self, scene_id: &str) -> Result<()> {
        self.rule_engine.trigger_rule(scene_id).await
    }
}

// 规则引擎（高级 API）
pub struct RuleEngine {
    // 实现细节...
}
```

---

## 🎯 总结

### 核心结论

**场景联动 = 简化的规则引擎**

### 实施方案

1. ✅ 实现统一的规则引擎（基于 Rhai）
2. ✅ 提供场景联动简化 API
3. ✅ 内部统一转换和执行

### 优势

- ✅ 避免重复开发
- ✅ 统一维护
- ✅ 渐进式学习（场景 → 规则）
- ✅ 灵活性和易用性兼顾

---

**建议**: 在规则引擎中同时实现场景联动功能，提供两层 API。

**维护者**: FLUX IOT Team  
**日期**: 2026-02-22
