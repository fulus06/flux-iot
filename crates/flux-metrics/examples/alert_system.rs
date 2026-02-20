use flux_metrics::{
    Alert, AlertAggregator, AlertDeduplicator, AlertEngine, AlertSeverity, Comparison,
    DingTalkNotifier, EmailNotifier, NotificationManager, ThresholdRule, WebhookNotifier,
};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    println!("=== FLUX IOT 告警系统示例 ===\n");

    // 1. 创建告警规则引擎
    println!("1. 创建告警规则引擎");
    let mut engine = AlertEngine::new();

    // 添加 CPU 告警规则
    engine.add_rule(Box::new(
        ThresholdRule::new(
            "high_cpu".to_string(),
            AlertSeverity::Warning,
            0.8,
            Comparison::GreaterThan,
        )
        .with_label("component".to_string(), "system".to_string()),
    ));

    // 添加内存告警规则
    engine.add_rule(Box::new(
        ThresholdRule::new(
            "high_memory".to_string(),
            AlertSeverity::Critical,
            0.9,
            Comparison::GreaterThan,
        )
        .with_label("component".to_string(), "system".to_string()),
    ));

    println!("已添加 {} 条告警规则\n", engine.rule_count());

    // 2. 创建通知管理器
    println!("2. 创建通知管理器");
    let mut notification_manager = NotificationManager::new();

    // 添加 Webhook 通知
    notification_manager.add_notifier(Box::new(WebhookNotifier::new(
        "https://example.com/webhook".to_string(),
    )));

    // 添加钉钉通知
    notification_manager.add_notifier(Box::new(DingTalkNotifier::new(
        "https://oapi.dingtalk.com/robot/send?access_token=xxx".to_string(),
    )));

    // 添加邮件通知
    notification_manager.add_notifier(Box::new(EmailNotifier::new(
        "smtp.example.com".to_string(),
        "alert@example.com".to_string(),
        vec!["admin@example.com".to_string()],
    )));

    println!("已添加 {} 个通知渠道\n", notification_manager.notifier_count());

    // 3. 创建告警聚合器和去重器
    println!("3. 创建告警聚合器和去重器");
    let aggregator = AlertAggregator::new(60, 300); // 60秒静默期，300秒聚合窗口
    let deduplicator = AlertDeduplicator::new(120); // 120秒去重窗口
    println!("告警聚合器已创建（静默期: 60s, 聚合窗口: 300s）");
    println!("告警去重器已创建（去重窗口: 120s）\n");

    // 4. 模拟告警场景
    println!("4. 模拟告警场景\n");

    // 场景 1: CPU 使用率过高
    println!("场景 1: CPU 使用率过高 (0.85)");
    let alerts = engine.evaluate("high_cpu", 0.85).await;
    for alert in &alerts {
        // 检查是否应该去重
        if !deduplicator.is_duplicate(alert).await {
            // 检查是否应该静默
            if !aggregator.should_silence(alert).await {
                println!("  ✓ 触发告警: {}", alert.name);
                notification_manager.notify(alert).await;
                
                // 记录告警
                deduplicator.record_alert(alert.clone()).await;
                aggregator.record_alert(alert).await;
            } else {
                println!("  ⊘ 告警被静默: {}", alert.name);
            }
        } else {
            println!("  ⊘ 告警被去重: {}", alert.name);
        }
    }

    sleep(Duration::from_secs(1)).await;

    // 场景 2: 重复的 CPU 告警（应该被静默）
    println!("\n场景 2: 重复的 CPU 告警（应该被静默）");
    let alerts = engine.evaluate("high_cpu", 0.85).await;
    if alerts.is_empty() {
        println!("  ⊘ 告警已存在，未重复触发");
    }

    sleep(Duration::from_secs(1)).await;

    // 场景 3: 内存使用率过高
    println!("\n场景 3: 内存使用率过高 (0.95)");
    let alerts = engine.evaluate("high_memory", 0.95).await;
    for alert in &alerts {
        if !deduplicator.is_duplicate(alert).await {
            if !aggregator.should_silence(alert).await {
                println!("  ✓ 触发告警: {} (严重程度: {:?})", alert.name, alert.severity);
                notification_manager.notify(alert).await;
                
                deduplicator.record_alert(alert.clone()).await;
                aggregator.record_alert(alert).await;
            }
        }
    }

    sleep(Duration::from_secs(1)).await;

    // 场景 4: CPU 恢复正常
    println!("\n场景 4: CPU 恢复正常 (0.5)");
    let alerts = engine.evaluate("high_cpu", 0.5).await;
    if alerts.is_empty() {
        println!("  ✓ CPU 使用率恢复正常，告警已解决");
    }

    // 5. 查看活跃告警
    println!("\n5. 当前活跃告警:");
    let active_alerts = engine.get_active_alerts().await;
    if active_alerts.is_empty() {
        println!("  无活跃告警");
    } else {
        for alert in active_alerts {
            println!("  - {} (严重程度: {:?})", alert.name, alert.severity);
        }
    }

    // 6. 查看告警历史
    println!("\n6. 告警历史 (最近 10 条):");
    let history = engine.get_alert_history(10).await;
    for alert in history {
        println!(
            "  - {} | 状态: {:?} | 触发时间: {}",
            alert.name, alert.state, alert.fired_at
        );
    }

    println!("\n=== 示例完成 ===");
}
