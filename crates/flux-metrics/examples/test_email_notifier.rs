use flux_metrics::{
    Alert, AlertSeverity, EmailConfig, EmailNotifier, Notifier,
};
use chrono::Utc;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 邮件通知器测试 ===\n");

    // 从环境变量读取配置
    let smtp_server = std::env::var("SMTP_SERVER")
        .unwrap_or_else(|_| "smtp.gmail.com".to_string());
    let smtp_user = std::env::var("SMTP_USER")
        .unwrap_or_else(|_| "your-email@gmail.com".to_string());
    let smtp_password = std::env::var("SMTP_PASSWORD")
        .unwrap_or_else(|_| "your-app-password".to_string());
    let to_email = std::env::var("TO_EMAIL")
        .unwrap_or_else(|_| "recipient@example.com".to_string());

    println!("配置信息:");
    println!("  SMTP 服务器: {}", smtp_server);
    println!("  发件人: {}", smtp_user);
    println!("  收件人: {}", to_email);
    println!();

    // 创建邮件配置
    let email_config = EmailConfig::new(
        smtp_server,
        smtp_user.clone(),
        smtp_user.clone(),
        smtp_password,
    );

    // 创建邮件通知器
    let notifier = EmailNotifier::new(email_config, vec![to_email]);

    // 创建测试告警
    let alert = Alert {
        id: "test-alert-001".to_string(),
        title: "系统测试告警".to_string(),
        message: "这是一条测试告警消息，用于验证邮件通知功能是否正常工作。".to_string(),
        severity: AlertSeverity::Warning,
        source: "flux-metrics-test".to_string(),
        timestamp: Utc::now(),
        labels: {
            let mut labels = HashMap::new();
            labels.insert("test".to_string(), "true".to_string());
            labels.insert("environment".to_string(), "development".to_string());
            labels
        },
    };

    println!("发送测试告警...");
    println!("  标题: {}", alert.title);
    println!("  级别: {:?}", alert.severity);
    println!("  消息: {}", alert.message);
    println!();

    // 发送邮件
    match notifier.send(&alert).await {
        Ok(_) => {
            println!("✅ 邮件发送成功！");
            println!();
            println!("请检查收件箱确认邮件已送达。");
        }
        Err(e) => {
            println!("❌ 邮件发送失败: {}", e);
            println!();
            println!("提示:");
            println!("1. 检查 SMTP 服务器配置是否正确");
            println!("2. 检查用户名和密码是否正确");
            println!("3. 如果使用 Gmail，需要使用应用专用密码");
            println!("4. 检查网络连接");
            println!();
            println!("环境变量设置示例:");
            println!("  export SMTP_SERVER=smtp.gmail.com");
            println!("  export SMTP_USER=your-email@gmail.com");
            println!("  export SMTP_PASSWORD=your-app-password");
            println!("  export TO_EMAIL=recipient@example.com");
        }
    }

    println!();
    println!("=== 测试完成 ===");

    Ok(())
}
