# 阶段 6：规则引擎 - 最终决策

> **决策日期**: 2026-02-22  
> **版本**: v3.0.0 (最终版)  
> **状态**: ✅ **方案确定**

---

## 🎯 最终决策

### 统一为规则引擎

**决定**: 用**规则引擎**完全替代**场景联动**

**理由**:
1. ✅ **统一性** - 只有一种概念，降低学习成本
2. ✅ **简化性** - 减少代码和维护成本
3. ✅ **强大性** - 规则引擎可以实现所有场景联动功能
4. ✅ **一致性** - 用户只需要学习 Rhai 脚本

---

## 📊 对比分析

### 之前的方案（两种概念）

```
场景联动 (Scene Linkage)
  ↓ 简单场景
  
规则引擎 (Rule Engine)
  ↓ 复杂规则
```

**问题**:
- ❌ 概念重复
- ❌ 用户困惑（何时用场景？何时用规则？）
- ❌ 代码重复
- ❌ 维护成本高

---

### 新方案（统一概念）

```
规则引擎 (Rule Engine)
  ↓ 使用 Rhai 脚本
  ↓ 可简单可复杂
  ↓ 统一执行
```

**优势**:
- ✅ 概念统一
- ✅ 用户清晰
- ✅ 代码简洁
- ✅ 易于维护

---

## 💡 如何用规则引擎实现"场景"？

### 原场景联动：回家模式

**旧方式（场景 API）**:
```yaml
场景: 回家模式
动作:
  - 开客厅灯
  - 开空调
  - 播放音乐
```

**新方式（规则引擎）**:
```rust
// 规则: 回家模式
// 触发: 手动触发

control_device("light_living_room", "turn_on", #{brightness: 80});
control_device("ac_001", "turn_on", #{temperature: 24});
control_device("speaker_001", "play", #{playlist: "favorites"});

log("info", "回家模式已激活");
```

**对比**:
- ✅ 同样简单
- ✅ 更灵活（可以加条件判断）
- ✅ 统一的 Rhai 语法

---

### 原场景联动：离家模式

**新方式（规则引擎）**:
```rust
// 规则: 离家模式
// 触发: 手动触发

// 关闭所有灯光
control_device("light_living_room", "turn_off", #{});
control_device("light_bedroom", "turn_off", #{});
control_device("light_kitchen", "turn_off", #{});

// 关闭空调
control_device("ac_001", "turn_off", #{});

// 启动安防
control_device("security_system", "arm", #{mode: "away"});

// 发送通知
send_notification("push", "离家模式", "已启动离家模式，安防系统已开启");

log("info", "离家模式已激活");
```

---

### 原场景联动：睡眠模式

**新方式（规则引擎 + 智能判断）**:
```rust
// 规则: 睡眠模式
// 触发: 手动触发

// 获取当前时间
let hour = now().hour();

// 关闭所有灯光
control_device("light_living_room", "turn_off", #{});
control_device("light_bedroom", "turn_off", #{});

// 关闭窗帘
control_device("curtain_bedroom", "close", #{});

// 设置空调（根据季节智能调节）
let month = now().month();
if month >= 6 && month <= 9 {
    // 夏季：制冷模式
    control_device("ac_bedroom", "set", #{
        mode: "cool",
        temperature: 26,
        fan: "low"
    });
} else if month >= 12 || month <= 2 {
    // 冬季：制热模式
    control_device("ac_bedroom", "set", #{
        mode: "heat",
        temperature: 20,
        fan: "low"
    });
}

// 设置静音模式
control_device("speaker_all", "set_volume", #{volume: 0});

log("info", "睡眠模式已激活");
```

**优势**: 比原场景联动更智能！

---

## 🏗️ 简化后的架构

### 最终架构

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

**特点**:
- ✅ 层次清晰
- ✅ 职责单一
- ✅ 易于扩展

---

## 📋 核心数据模型

### 规则模型（最终版）

```rust
/// 规则定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// 规则 ID
    pub id: String,
    
    /// 规则名称
    pub name: String,
    
    /// 规则描述
    pub description: String,
    
    /// 规则分类/标签
    pub tags: Vec<String>,
    
    /// 是否启用
    pub enabled: bool,
    
    /// 触发器
    pub trigger: RuleTrigger,
    
    /// Rhai 脚本
    pub script: String,
    
    /// 优先级（1-100）
    pub priority: i32,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    
    /// 创建者
    pub created_by: String,
}

/// 触发器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleTrigger {
    /// 手动触发（替代原场景的一键触发）
    Manual,
    
    /// 设备事件触发
    DeviceEvent {
        device_id: String,
        event_type: String,
    },
    
    /// 数据变化触发
    DataChange {
        device_id: String,
        metric: Option<String>,
    },
    
    /// 定时触发
    Schedule {
        cron: String,
    },
}
```

---

## 🎯 规则分类建议

### 使用标签（Tags）区分规则类型

```rust
// 原"场景联动"类规则
Rule {
    name: "回家模式",
    tags: vec!["scene", "home", "manual"],  // 标记为场景类
    trigger: RuleTrigger::Manual,
    script: "...",
}

// 自动化规则
Rule {
    name: "智能温控",
    tags: vec!["automation", "temperature"],
    trigger: RuleTrigger::DataChange { ... },
    script: "...",
}

// 告警规则
Rule {
    name: "高温告警",
    tags: vec!["alert", "temperature"],
    trigger: RuleTrigger::DataChange { ... },
    script: "...",
}

// 定时任务
Rule {
    name: "每日报告",
    tags: vec!["schedule", "report"],
    trigger: RuleTrigger::Schedule { cron: "0 8 * * *" },
    script: "...",
}
```

**优势**:
- ✅ 灵活分类
- ✅ 易于检索
- ✅ 用户可自定义标签

---

## 💻 UI 设计建议

### 规则列表界面

```
┌─────────────────────────────────────────┐
│  规则管理                    [+ 新建规则] │
├─────────────────────────────────────────┤
│  筛选: [全部] [场景] [自动化] [告警]     │
├─────────────────────────────────────────┤
│  📋 回家模式                  [手动触发]  │
│     标签: scene, home                    │
│     最后执行: 2小时前                    │
├─────────────────────────────────────────┤
│  🌡️ 智能温控                 [自动执行]  │
│     标签: automation, temperature        │
│     触发: 温度变化                       │
├─────────────────────────────────────────┤
│  🚨 高温告警                  [自动执行]  │
│     标签: alert, temperature             │
│     触发: 温度 > 80°C                    │
└─────────────────────────────────────────┘
```

**特点**:
- ✅ 统一界面
- ✅ 标签筛选
- ✅ 清晰分类

---

## ✅ 最终优势

### 1. 概念统一 ✅

**之前**: 场景 + 规则（两个概念）  
**现在**: 规则（一个概念）

### 2. 代码简化 ✅

**减少代码量**: ~300 行（不需要场景 API）  
**最终代码量**: ~1,500 行

### 3. 学习成本降低 ✅

**之前**: 学习场景 API + 规则 API  
**现在**: 只学习 Rhai 脚本

### 4. 功能更强大 ✅

**之前**: 场景只能固定流程  
**现在**: 规则可以加条件判断，更智能

### 5. 维护成本降低 ✅

**之前**: 维护两套系统  
**现在**: 维护一套系统

---

## 📋 实施计划（更新）

### 工期：5 天（减少 1 天）

| 天数 | 任务 | 代码量 |
|------|------|--------|
| 1 | 规则模型和存储 | ~250 行 |
| 2-3 | 规则引擎核心 | ~600 行 |
| 4 | 触发器系统 | ~400 行 |
| 5 | 测试和文档 | ~250 行 |

**总计**: ~1,500 行（减少 300 行）

---

## 🚀 下一步

**方案最终确定**: 统一为规则引擎

**优势**:
- ✅ 概念统一
- ✅ 代码简化
- ✅ 功能强大
- ✅ 易于维护

**准备开始实施？**

---

**维护者**: FLUX IOT Team  
**决策日期**: 2026-02-22  
**状态**: ✅ **最终方案确定**
