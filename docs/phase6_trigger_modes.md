# 规则引擎触发方式设计

> **设计日期**: 2026-02-22  
> **版本**: v1.0.0  
> **状态**: ✅ **最终确定**

---

## 🎯 三种触发方式

### 1. 手动模式 (Manual Mode)

**定义**: 用户主动触发规则执行

**应用场景**:
- 一键场景（回家模式、离家模式、睡眠模式）
- 临时操作（会议模式、演示模式）
- 测试规则

**触发方式**:
- UI 按钮点击
- API 调用
- 语音命令
- 快捷方式

**示例**:
```rust
// 规则: 回家模式
// 触发: 手动

control_device("light_living_room", "turn_on", #{brightness: 80});
control_device("ac_001", "turn_on", #{temperature: 24});
control_device("speaker_001", "play", #{playlist: "favorites"});

send_notification("push", "回家模式", "已激活回家模式");
log("info", "回家模式已手动触发");
```

---

### 2. 定时模式 (Schedule Mode)

**定义**: 按照时间计划自动触发规则

**应用场景**:
- 定时任务（每天开关灯、定时报告）
- 周期性操作（每周清理、每月统计）
- 特定时间场景（工作日早晨、周末晚上）

**触发方式**:
- Cron 表达式
- 固定时间点
- 时间范围

**Cron 表达式格式**:
```
秒 分 时 日 月 周

示例:
0 8 * * *        # 每天 8:00
0 0 * * 1        # 每周一 0:00
0 12 1 * *       # 每月 1 号 12:00
*/5 * * * *      # 每 5 分钟
0 9 * * 1-5      # 工作日 9:00
```

**示例 1: 每天定时开启热水器**
```rust
// 规则: 定时开启热水器
// 触发: 定时 (每天 6:30)
// Cron: 0 30 6 * * *

control_device("water_heater", "turn_on", #{
    temperature: 60,
    mode: "eco"
});

log("info", "热水器已定时开启");
```

**示例 2: 工作日早晨场景**
```rust
// 规则: 工作日早晨场景
// 触发: 定时 (工作日 7:00)
// Cron: 0 0 7 * * 1-5

// 开启卧室灯光
control_device("light_bedroom", "turn_on", #{brightness: 50});

// 打开窗帘
control_device("curtain_bedroom", "open", #{});

// 播放新闻
control_device("speaker_bedroom", "play", #{
    source: "news",
    volume: 30
});

log("info", "工作日早晨场景已触发");
```

**示例 3: 每日能耗报告**
```rust
// 规则: 每日能耗报告
// 触发: 定时 (每天 8:00)
// Cron: 0 0 8 * * *

// 查询昨天的能耗数据
let yesterday = date_add(now(), -1, "day");
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
`;

// 发送邮件
send_email(#{
    to: "admin@example.com",
    subject: "能耗日报",
    body: report
});

log("info", "每日能耗报告已发送");
```

---

### 3. 条件模式 (Condition Mode)

**定义**: 根据物联网设备反馈自动触发规则

**应用场景**:
- 设备状态变化（门开、灯亮、温度变化）
- 数据阈值告警（温度过高、湿度过低）
- 设备联动（门开灯亮、人来空调开）
- 异常检测（设备离线、数据异常）

**触发条件**:
- 设备事件（状态变化、告警事件）
- 数据变化（指标更新、阈值触发）
- 设备上线/离线

**示例 1: 高温告警**
```rust
// 规则: 高温告警
// 触发: 条件 (温度传感器数据变化)
// 条件: device_id = "sensor_001", metric = "temperature"

let temp = device.temperature;

// 条件判断
if temp > 80.0 {
    // 发送紧急通知
    send_notification("urgent", "高温告警", 
        `设备温度过高: ${temp}°C，请立即检查！`);
    
    // 控制风扇
    control_device("fan_001", "turn_on", #{speed: "high"});
    
    // 记录告警事件
    record_event("high_temperature_alert", #{
        device_id: "sensor_001",
        temperature: temp,
        timestamp: now()
    });
    
    log("warn", `高温告警触发: ${temp}°C`);
}
```

**示例 2: 门禁联动照明**
```rust
// 规则: 门禁联动照明
// 触发: 条件 (门禁状态变化)
// 条件: device_id = "door_001", event_type = "status_change"

let door_status = device.status;
let current_hour = now().hour();

// 门打开时
if door_status == "open" {
    // 根据时间判断是否开灯
    if current_hour >= 18 || current_hour <= 6 {
        // 晚上或早晨，开启走廊灯
        control_device("light_corridor", "turn_on", #{
            brightness: 80,
            duration: 300  // 5分钟后自动关闭
        });
        
        log("info", "门开启，已自动开灯");
    }
    
    // 记录进出日志
    record_event("door_access", #{
        door_id: "door_001",
        action: "open",
        timestamp: now()
    });
}
```

**示例 3: 智能空调控制**
```rust
// 规则: 智能空调控制
// 触发: 条件 (温度或人体传感器数据变化)
// 条件: device_id = "sensor_room", metric = null (任何数据变化)

let temp = device.temperature;
let humidity = device.humidity;
let occupancy = device.occupancy;

// 有人在房间
if occupancy {
    // 温度过高
    if temp > 26.0 {
        control_device("ac_001", "set", #{
            mode: "cool",
            temperature: 24,
            fan: "auto"
        });
        log("info", `温度 ${temp}°C，已开启制冷`);
    }
    // 温度过低
    else if temp < 20.0 {
        control_device("ac_001", "set", #{
            mode: "heat",
            temperature: 22,
            fan: "auto"
        });
        log("info", `温度 ${temp}°C，已开启制热`);
    }
    
    // 湿度控制
    if humidity > 70.0 {
        control_device("dehumidifier_001", "turn_on", #{});
        log("info", `湿度 ${humidity}%，已开启除湿`);
    }
} else {
    // 无人时关闭空调（节能）
    control_device("ac_001", "turn_off", #{});
    log("info", "房间无人，已关闭空调");
}
```

**示例 4: 连续异常检测**
```rust
// 规则: 连续异常检测
// 触发: 条件 (设备振动数据变化)
// 条件: device_id = "machine_001", metric = "vibration"

let vibration = device.vibration;

// 检查振动是否超过阈值
if vibration > 5.0 {
    // 记录异常事件
    record_event("high_vibration", #{
        device_id: "machine_001",
        value: vibration,
        timestamp: now()
    });
    
    // 检查最近 5 分钟内的异常次数
    let count = count_events("high_vibration", "5min");
    
    // 连续 3 次异常
    if count >= 3 {
        // 发送紧急告警
        send_notification("urgent", "设备异常", 
            `设备振动异常，最近5分钟内发生 ${count} 次，振动值: ${vibration}`);
        
        // 创建工单
        create_ticket(#{
            title: "设备振动异常",
            device_id: "machine_001",
            priority: "high",
            description: `振动值: ${vibration}, 异常次数: ${count}`
        });
        
        // 更新设备状态
        update_device_status("machine_001", "fault");
        
        log("error", `设备异常: 连续 ${count} 次振动超标`);
    }
}
```

**示例 5: 设备离线告警**
```rust
// 规则: 设备离线告警
// 触发: 条件 (设备状态变化)
// 条件: device_id = "critical_device", event_type = "offline"

// 设备离线
send_notification("urgent", "设备离线", 
    `关键设备 ${device.name} 已离线，请立即检查！`);

// 创建工单
create_ticket(#{
    title: "设备离线",
    device_id: device.id,
    priority: "critical",
    description: `设备 ${device.name} 在 ${now()} 离线`
});

// 记录事件
record_event("device_offline", #{
    device_id: device.id,
    device_name: device.name,
    timestamp: now()
});

log("error", `设备 ${device.name} 离线`);
```

---

## 📊 触发方式对比

| 触发方式 | 触发源 | 自动化 | 应用场景 | 复杂度 |
|---------|--------|--------|---------|--------|
| **手动模式** | 用户操作 | ❌ | 一键场景、临时操作 | 低 |
| **定时模式** | 时间计划 | ✅ | 定时任务、周期操作 | 中 |
| **条件模式** | 设备反馈 | ✅ | 智能联动、告警检测 | 高 |

---

## 🔧 数据模型

### 触发器定义

```rust
/// 规则触发器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleTrigger {
    /// 手动触发
    Manual,
    
    /// 定时触发
    Schedule {
        /// Cron 表达式
        cron: String,
    },
    
    /// 条件触发 - 设备事件
    DeviceEvent {
        /// 设备 ID
        device_id: String,
        /// 事件类型 (status_change, online, offline, alert)
        event_type: String,
    },
    
    /// 条件触发 - 数据变化
    DataChange {
        /// 设备 ID
        device_id: String,
        /// 指标名称 (None 表示任何指标变化都触发)
        metric: Option<String>,
    },
}
```

---

## 💡 触发器组合使用

### 示例：智能场景组合

```rust
// 规则 1: 手动触发 - 离家模式
// 触发: Manual

control_device("light_all", "turn_off", #{});
control_device("ac_all", "turn_off", #{});
control_device("security", "arm", #{mode: "away"});

// 规则 2: 定时触发 - 每天晚上自动离家模式
// 触发: Schedule { cron: "0 0 23 * * *" }

// 检查是否有人在家
let occupancy = read_device("sensor_living_room", "occupancy");

if !occupancy {
    // 无人时自动执行离家模式
    control_device("light_all", "turn_off", #{});
    control_device("ac_all", "turn_off", #{});
    control_device("security", "arm", #{mode: "away"});
    
    send_notification("push", "自动离家", "已自动启动离家模式");
}

// 规则 3: 条件触发 - 门打开时检查安防
// 触发: DeviceEvent { device_id: "door_main", event_type: "status_change" }

if device.status == "open" {
    let security_status = read_device("security", "status");
    
    if security_status == "armed" {
        // 安防启动时门被打开，发送告警
        send_notification("urgent", "安防告警", "安防启动时门被打开！");
        control_device("alarm", "trigger", #{});
    }
}
```

---

## ✅ 实施要点

### 1. 手动模式实现

```rust
impl RuleEngine {
    /// 手动触发规则
    pub async fn trigger_manual(&self, rule_id: &str) -> Result<()> {
        let rule = self.get_rule(rule_id).await?;
        
        // 检查触发器类型
        if !matches!(rule.trigger, RuleTrigger::Manual) {
            return Err(anyhow!("Rule is not manual trigger"));
        }
        
        // 执行规则
        self.execute_rule(rule_id, RuleContext::default()).await
    }
}
```

---

### 2. 定时模式实现

```rust
use tokio_cron_scheduler::{JobScheduler, Job};

impl RuleEngine {
    /// 注册定时规则
    pub async fn register_schedule(&self, rule: &Rule) -> Result<()> {
        if let RuleTrigger::Schedule { cron } = &rule.trigger {
            let rule_id = rule.id.clone();
            let engine = self.clone();
            
            let job = Job::new_async(cron, move |_uuid, _lock| {
                let rule_id = rule_id.clone();
                let engine = engine.clone();
                
                Box::pin(async move {
                    if let Err(e) = engine.execute_rule(&rule_id, RuleContext::default()).await {
                        error!("Failed to execute scheduled rule: {}", e);
                    }
                })
            })?;
            
            self.scheduler.add(job).await?;
        }
        
        Ok(())
    }
}
```

---

### 3. 条件模式实现

```rust
impl RuleEngine {
    /// 处理设备事件
    pub async fn handle_device_event(&self, device_id: &str, event_type: &str, data: Value) -> Result<()> {
        // 查找匹配的规则
        let rules = self.find_rules_by_trigger(RuleTrigger::DeviceEvent {
            device_id: device_id.to_string(),
            event_type: event_type.to_string(),
        }).await?;
        
        // 执行所有匹配的规则
        for rule in rules {
            let context = RuleContext {
                device_data: hashmap!{
                    "id" => device_id.into(),
                    "event_type" => event_type.into(),
                    "data" => data.clone(),
                },
                ..Default::default()
            };
            
            self.execute_rule(&rule.id, context).await?;
        }
        
        Ok(())
    }
    
    /// 处理数据变化
    pub async fn handle_data_change(&self, device_id: &str, metric: &str, value: Value) -> Result<()> {
        // 查找匹配的规则
        let rules = self.find_rules_by_data_change(device_id, metric).await?;
        
        // 执行所有匹配的规则
        for rule in rules {
            let context = RuleContext {
                device_data: hashmap!{
                    "id" => device_id.into(),
                    metric => value.clone(),
                },
                ..Default::default()
            };
            
            self.execute_rule(&rule.id, context).await?;
        }
        
        Ok(())
    }
}
```

---

## 🚀 总结

### 三种触发方式

1. **手动模式** - 用户主动触发
2. **定时模式** - 按时间计划触发
3. **条件模式** - 根据物联网反馈触发

### 覆盖场景

- ✅ 一键场景（手动）
- ✅ 定时任务（定时）
- ✅ 智能联动（条件）
- ✅ 告警检测（条件）
- ✅ 异常处理（条件）

### 实施优先级

1. **第一优先**: 条件模式（最常用，最核心）
2. **第二优先**: 手动模式（简单，易实现）
3. **第三优先**: 定时模式（需要 Cron 调度器）

---

**维护者**: FLUX IOT Team  
**设计日期**: 2026-02-22  
**状态**: ✅ **触发方式设计完成**
