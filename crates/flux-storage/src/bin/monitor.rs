use anyhow::Result;
use flux_notify::{NotifyChannel, NotifyLevel, NotifyManager};
use flux_notify::providers::{
    DingTalkConfig, DingTalkNotifier, EmailConfig, EmailNotifier,
};
use flux_storage::monitor::{MonitorConfig, MonitorService};
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    info!("Starting flux-storage-monitor service");

    // 加载配置
    let config = MonitorConfig::load("config/storage_monitor.toml")
        .unwrap_or_else(|_| {
            info!("Using default configuration");
            MonitorConfig::default()
        });

    // 创建通知管理器
    let notify_manager = Arc::new(NotifyManager::new(NotifyLevel::Warning));

    // 注册通知器（从环境变量或配置文件读取）
    register_notifiers(&notify_manager).await;

    // 创建监控服务
    let service = Arc::new(
        MonitorService::new(
            config.storage_pools,
            notify_manager,
            config.check_interval_secs,
            config.alert_dedup_minutes,
            config.send_recovery_alerts,
            config.alert_on_status_change,
            config.enable_lifecycle_gc,
            config.retention_days,
            config.max_capacity_gb,
            config.gc_trigger_usage_percent,
            config.gc_target_usage_percent,
            config.notify_gc_results,
            config.telemetry_enabled,
            config.telemetry_endpoint,
            config.telemetry_timeout_ms,
        )
        .await?,
    );

    info!("Monitor service initialized successfully");

    // 启动存储健康检查任务
    let health_handle = service.start_storage_health_check();

    // 启动监控任务
    let monitor_handle = service.clone().start_monitoring_task();

    tokio::signal::ctrl_c().await?;

    health_handle.shutdown().await;
    monitor_handle.shutdown().await;

    Ok(())
}

/// 注册通知器
async fn register_notifiers(notify_manager: &Arc<NotifyManager>) {
    // 邮件通知（从环境变量读取）
    if let (Ok(smtp_host), Ok(smtp_user), Ok(smtp_pass), Ok(from), Ok(to)) = (
        std::env::var("SMTP_HOST"),
        std::env::var("SMTP_USER"),
        std::env::var("SMTP_PASS"),
        std::env::var("SMTP_FROM"),
        std::env::var("SMTP_TO"),
    ) {
        let email_config = EmailConfig {
            smtp_host,
            smtp_port: std::env::var("SMTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(587),
            username: smtp_user,
            password: smtp_pass,
            from,
            to: vec![to],
        };

        let email_notifier = EmailNotifier::new(email_config);
        notify_manager
            .register(NotifyChannel::Email, Box::new(email_notifier))
            .await;
        info!("Email notifier registered");
    }

    // 钉钉通知
    if let Ok(webhook_url) = std::env::var("DINGTALK_WEBHOOK") {
        let dingtalk_config = DingTalkConfig {
            webhook_url,
            secret: std::env::var("DINGTALK_SECRET").ok(),
        };

        let dingtalk_notifier = DingTalkNotifier::new(dingtalk_config);
        notify_manager
            .register(NotifyChannel::DingTalk, Box::new(dingtalk_notifier))
            .await;
        info!("DingTalk notifier registered");
    }

    // 如果没有注册任何通知器，输出警告
    info!("Notifier registration completed");
}
