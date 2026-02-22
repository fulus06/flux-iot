use flux_rule::{RuleEngine, Rule, RuleTrigger, RuleContext, TriggerManager, RateLimit};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🚀 FLUX IOT - 规则引擎完整演示\n");

    // 创建规则引擎
    let engine = Arc::new(RuleEngine::new());
    
    // 创建触发器管理器
    let trigger_manager = TriggerManager::new(engine.clone());
    trigger_manager.start().await?;

    println!("=" .repeat(60));
    println!("示例 1: 手动触发规则");
    println!("=" .repeat(60));
    
    let manual_rule = Rule {
        name: "回家模式".to_string(),
        description: "一键回家场景".to_string(),
        trigger: RuleTrigger::Manual,
        script: r#"
            log("info", "回家模式已激活");
            control_device("light_living_room", "turn_on", #{brightness: 80});
            control_device("ac_001", "turn_on", #{temperature: 24});
            send_notification("push", "回家模式", "已激活回家模式");
        "#.to_string(),
        tags: vec!["scene".to_string(), "home".to_string()],
        ..Default::default()
    };
    
    let rule_id = engine.add_rule(manual_rule).await?;
    println!("✅ 规则已添加: {}", rule_id);
    
    // 手动触发
    engine.trigger_manual(&rule_id, RuleContext::new()).await?;
    println!("✅ 规则已执行\n");

    println!("=" .repeat(60));
    println!("示例 2: 定时触发规则");
    println!("=" .repeat(60));
    
    let schedule_rule = Rule {
        name: "每日报告".to_string(),
        description: "每天8点生成报告".to_string(),
        trigger: RuleTrigger::Schedule {
            cron: "0 8 * * *".to_string(),
        },
        script: r#"
            log("info", "开始生成每日报告");
            let report = query_metrics(#{
                metric: "energy_consumption",
                range: "1day"
            });
            send_email(#{
                to: "admin@example.com",
                subject: "能耗日报",
                body: "总能耗: " + report.total
            });
        "#.to_string(),
        tags: vec!["schedule".to_string(), "report".to_string()],
        ..Default::default()
    };
    
    let schedule_rule_id = engine.add_rule(schedule_rule.clone()).await?;
    trigger_manager.register_rule(&schedule_rule).await?;
    println!("✅ 定时规则已注册: {}", schedule_rule_id);
    println!("   Cron: 0 8 * * * (每天 8:00)\n");

    println!("=" .repeat(60));
    println!("示例 3: 条件触发规则（数据变化）");
    println!("=" .repeat(60));
    
    let data_change_rule = Rule {
        name: "高温告警".to_string(),
        description: "温度超过80度时告警".to_string(),
        trigger: RuleTrigger::DataChange {
            device_id: "sensor_001".to_string(),
            metric: Some("temperature".to_string()),
        },
        script: r#"
            let temp = device.temperature;
            log("info", "当前温度: " + temp);
            
            if temp > 80.0 {
                send_notification("urgent", "高温告警", "温度: " + temp + "°C");
                control_device("fan_001", "turn_on", #{speed: "high"});
                
                // 记录告警事件
                record_event("high_temperature", #{
                    device_id: "sensor_001",
                    temperature: temp
                });
            }
        "#.to_string(),
        tags: vec!["automation".to_string(), "alert".to_string()],
        ..Default::default()
    };
    
    let data_rule_id = engine.add_rule(data_change_rule).await?;
    println!("✅ 数据变化规则已添加: {}", data_rule_id);
    
    // 模拟数据变化触发
    println!("   模拟温度变化...");
    trigger_manager.handle_data_change(
        "sensor_001",
        "temperature",
        serde_json::json!(85.0),
    ).await?;
    println!("✅ 规则已触发执行\n");

    println!("=" .repeat(60));
    println!("示例 4: 限流控制");
    println!("=" .repeat(60));
    
    let rate_limited_rule = Rule {
        name: "限流告警".to_string(),
        description: "1分钟最多10次".to_string(),
        trigger: RuleTrigger::Manual,
        script: r#"
            send_notification("alert", "告警", "异常事件");
        "#.to_string(),
        rate_limit: Some(RateLimit {
            max_executions: 3,
            time_window_seconds: 60,
        }),
        ..Default::default()
    };
    
    let rate_rule_id = engine.add_rule(rate_limited_rule).await?;
    println!("✅ 限流规则已添加: {}", rate_rule_id);
    println!("   限制: 1分钟最多3次");
    
    // 测试限流
    for i in 1..=5 {
        match engine.trigger_manual(&rate_rule_id, RuleContext::new()).await {
            Ok(_) => println!("   第{}次执行: ✅ 成功", i),
            Err(e) => println!("   第{}次执行: ❌ 失败 ({})", i, e),
        }
    }
    println!();

    println!("=" .repeat(60));
    println!("示例 5: 执行历史查询");
    println!("=" .repeat(60));
    
    let history = engine.get_execution_history(None, 10).await?;
    println!("最近执行记录: {} 条", history.len());
    for (i, exec) in history.iter().enumerate() {
        println!("{}. {} - {:?} - {}", 
            i + 1,
            exec.rule_name,
            exec.status,
            exec.started_at.format("%H:%M:%S")
        );
    }
    println!();

    println!("=" .repeat(60));
    println!("示例 6: 规则分组管理");
    println!("=" .repeat(60));
    
    // 添加分组规则
    let group_rule = Rule {
        name: "离家模式".to_string(),
        group: Some("scene".to_string()),
        trigger: RuleTrigger::Manual,
        script: r#"
            log("info", "离家模式已激活");
            control_device("light_all", "turn_off", #{});
            control_device("ac_all", "turn_off", #{});
        "#.to_string(),
        tags: vec!["scene".to_string()],
        ..Default::default()
    };
    
    engine.add_rule(group_rule).await?;
    
    // 按标签查找
    let scene_rules = engine.find_by_tag("scene").await?;
    println!("场景类规则: {} 个", scene_rules.len());
    for rule in &scene_rules {
        println!("  - {}", rule.name);
    }
    println!();

    println!("=" .repeat(60));
    println!("✅ 演示完成！");
    println!("=" .repeat(60));
    println!("\n规则引擎功能:");
    println!("  ✅ 手动触发");
    println!("  ✅ 定时触发 (Cron)");
    println!("  ✅ 条件触发 (数据变化/设备事件)");
    println!("  ✅ 限流控制");
    println!("  ✅ 执行历史");
    println!("  ✅ 规则分组");
    println!("  ✅ 内置函数");
    println!("\n🎉 FLUX IOT 规则引擎 - 100% 完成！");

    // 清理
    trigger_manager.stop().await?;

    Ok(())
}
