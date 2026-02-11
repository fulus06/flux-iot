use metrics::{counter, gauge, histogram, describe_counter, describe_gauge, describe_histogram};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;

/// 初始化 Prometheus metrics exporter
pub fn init_metrics(addr: SocketAddr) -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .map_err(|e| anyhow::anyhow!("Failed to install Prometheus exporter: {}", e))?;
    
    // 描述所有指标
    describe_metrics();
    
    tracing::info!("Metrics exporter started on http://{}/metrics", addr);
    Ok(())
}

/// 描述所有指标
fn describe_metrics() {
    // 事件相关指标
    describe_counter!("flux_events_published_total", "Total number of events published");
    describe_counter!("flux_events_received_total", "Total number of events received");
    
    // 规则相关指标
    describe_counter!("flux_rules_executed_total", "Total number of rules executed");
    describe_counter!("flux_rules_triggered_total", "Total number of rules triggered");
    describe_counter!("flux_rules_failed_total", "Total number of rule execution failures");
    describe_gauge!("flux_rules_active", "Number of active rules");
    
    // 插件相关指标
    describe_counter!("flux_plugin_calls_total", "Total number of plugin calls");
    describe_counter!("flux_plugin_failures_total", "Total number of plugin failures");
    describe_histogram!("flux_plugin_duration_seconds", "Plugin execution duration in seconds");
    describe_gauge!("flux_plugins_loaded", "Number of loaded plugins");
    
    // HTTP API 相关指标
    describe_counter!("flux_http_requests_total", "Total number of HTTP requests");
    describe_histogram!("flux_http_request_duration_seconds", "HTTP request duration in seconds");
    
    // 系统相关指标
    describe_gauge!("flux_eventbus_capacity", "EventBus capacity");
    describe_gauge!("flux_database_connections", "Number of database connections");
}

/// 记录事件发布
pub fn record_event_published() {
    counter!("flux_events_published_total", 1);
}

/// 记录事件接收
pub fn record_event_received() {
    counter!("flux_events_received_total", 1);
}

/// 记录规则执行
pub fn record_rule_executed() {
    counter!("flux_rules_executed_total", 1);
}

/// 记录规则触发
pub fn record_rule_triggered() {
    counter!("flux_rules_triggered_total", 1);
}

/// 记录规则执行失败
pub fn record_rule_failed() {
    counter!("flux_rules_failed_total", 1);
}

/// 设置活跃规则数量
pub fn set_active_rules(count: usize) {
    gauge!("flux_rules_active", count as f64);
}

/// 记录插件调用
pub fn record_plugin_call() {
    counter!("flux_plugin_calls_total", 1);
}

/// 记录插件失败
pub fn record_plugin_failure() {
    counter!("flux_plugin_failures_total", 1);
}

/// 记录插件执行时间
pub fn record_plugin_duration(duration_secs: f64) {
    histogram!("flux_plugin_duration_seconds", duration_secs);
}

/// 设置已加载插件数量
pub fn set_loaded_plugins(count: usize) {
    gauge!("flux_plugins_loaded", count as f64);
}

/// 记录 HTTP 请求
pub fn record_http_request() {
    counter!("flux_http_requests_total", 1);
}

/// 记录 HTTP 请求时长
pub fn record_http_duration(duration_secs: f64) {
    histogram!("flux_http_request_duration_seconds", duration_secs);
}

/// 设置 EventBus 容量
pub fn set_eventbus_capacity(capacity: usize) {
    gauge!("flux_eventbus_capacity", capacity as f64);
}

/// 设置数据库连接数
pub fn set_database_connections(count: usize) {
    gauge!("flux_database_connections", count as f64);
}
