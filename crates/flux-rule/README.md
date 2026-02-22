# flux-rule

规则引擎 - FLUX IOT 平台的自动化决策系统

## 功能特性

### 核心功能 ✅
- ✅ 纯 Rhai 脚本引擎
- ✅ 三种触发方式（手动、定时、条件）
- ✅ 规则优先级
- ✅ 执行超时控制
- ✅ 规则限流
- ✅ 执行历史记录
- ✅ 测试模式
- ✅ 规则分组和标签
- ✅ 规则版本控制

### 三种触发方式

**1. 手动触发**
```rust
let rule = Rule {
    name: "回家模式".to_string(),
    trigger: RuleTrigger::Manual,
    script: r#"
        control_device("light_living_room", "turn_on", #{brightness: 80});
        control_device("ac_001", "turn_on", #{temperature: 24});
    "#.to_string(),
    ..Default::default()
};
```

**2. 定时触发**
```rust
let rule = Rule {
    name: "每日报告".to_string(),
    trigger: RuleTrigger::Schedule {
        cron: "0 8 * * *".to_string(),  // 每天 8:00
    },
    script: r#"
        let report = generate_daily_report();
        send_email("admin@example.com", "日报", report);
    "#.to_string(),
    ..Default::default()
};
```

**3. 条件触发**
```rust
let rule = Rule {
    name: "高温告警".to_string(),
    trigger: RuleTrigger::DataChange {
        device_id: "sensor_001".to_string(),
        metric: Some("temperature".to_string()),
    },
    script: r#"
        let temp = device.temperature;
        if temp > 80.0 {
            send_notification("urgent", "高温告警", `温度: ${temp}°C`);
            control_device("fan_001", "turn_on", #{speed: "high"});
        }
    "#.to_string(),
    ..Default::default()
};
```

## 使用示例

```rust
use flux_rule::{RuleEngine, Rule, RuleTrigger, RuleContext};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建规则引擎
    let engine = RuleEngine::new();
    
    // 添加规则
    let rule = Rule {
        name: "智能温控".to_string(),
        trigger: RuleTrigger::DataChange {
            device_id: "sensor_room".to_string(),
            metric: None,
        },
        script: r#"
            let temp = device.temperature;
            let occupancy = device.occupancy;
            
            if occupancy {
                if temp > 26.0 {
                    control_device("ac_001", "set", #{
                        mode: "cool",
                        temperature: 24
                    });
                }
            } else {
                control_device("ac_001", "turn_off", #{});
            }
        "#.to_string(),
        priority: 50,
        timeout_seconds: 30,
        ..Default::default()
    };
    
    let rule_id = engine.add_rule(rule).await?;
    
    // 手动触发规则
    let context = RuleContext::new();
    engine.trigger_manual(&rule_id, context).await?;
    
    // 查看执行历史
    let history = engine.get_execution_history(Some(&rule_id), 10).await?;
    println!("执行历史: {:?}", history);
    
    Ok(())
}
```

## 高级功能

### 规则限流
```rust
let rule = Rule {
    rate_limit: Some(RateLimit {
        max_executions: 10,
        time_window_seconds: 60,  // 1分钟最多10次
    }),
    ..Default::default()
};
```

### 测试模式
```rust
let test_result = engine.test_rule(&rule_id, mock_context).await?;
println!("测试结果: {:?}", test_result);
```

### 规则分组
```rust
// 按分组启用/禁用
engine.enable_group("scene", true).await?;

// 按标签查找
let rules = engine.find_by_tag("automation").await?;
```

## 许可证

MIT License
