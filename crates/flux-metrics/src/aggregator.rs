use crate::alert::{Alert, AlertSeverity, AlertState};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// 告警聚合器
pub struct AlertAggregator {
    // 指纹 -> 最后一次告警时间
    last_alert_time: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    // 静默期（秒）
    silence_duration: Duration,
    // 聚合窗口（秒）
    aggregation_window: Duration,
    // 聚合的告警
    aggregated_alerts: Arc<RwLock<HashMap<String, Vec<Alert>>>>,
}

impl AlertAggregator {
    pub fn new(silence_duration_secs: i64, aggregation_window_secs: i64) -> Self {
        Self {
            last_alert_time: Arc::new(RwLock::new(HashMap::new())),
            silence_duration: Duration::seconds(silence_duration_secs),
            aggregation_window: Duration::seconds(aggregation_window_secs),
            aggregated_alerts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查告警是否应该被静默
    pub async fn should_silence(&self, alert: &Alert) -> bool {
        let fingerprint = alert.fingerprint();
        let last_times = self.last_alert_time.read().await;

        if let Some(last_time) = last_times.get(&fingerprint) {
            let elapsed = Utc::now() - *last_time;
            if elapsed < self.silence_duration {
                debug!(
                    "Alert {} silenced (last fired {} seconds ago)",
                    alert.name,
                    elapsed.num_seconds()
                );
                return true;
            }
        }

        false
    }

    /// 记录告警发送时间
    pub async fn record_alert(&self, alert: &Alert) {
        let fingerprint = alert.fingerprint();
        let mut last_times = self.last_alert_time.write().await;
        last_times.insert(fingerprint, Utc::now());
    }

    /// 添加告警到聚合窗口
    pub async fn add_to_aggregation(&self, alert: Alert) {
        let key = format!("{:?}", alert.severity);
        let mut aggregated = self.aggregated_alerts.write().await;
        aggregated.entry(key).or_insert_with(Vec::new).push(alert);
    }

    /// 获取聚合的告警并清空
    pub async fn get_aggregated_alerts(&self) -> HashMap<AlertSeverity, Vec<Alert>> {
        let mut aggregated = self.aggregated_alerts.write().await;
        let mut result = HashMap::new();

        for (severity_str, alerts) in aggregated.drain() {
            let severity = match severity_str.as_str() {
                "Info" => AlertSeverity::Info,
                "Warning" => AlertSeverity::Warning,
                "Critical" => AlertSeverity::Critical,
                _ => continue,
            };
            result.insert(severity, alerts);
        }

        result
    }

    /// 清理过期的静默记录
    pub async fn cleanup_expired(&self) {
        let mut last_times = self.last_alert_time.write().await;
        let now = Utc::now();
        let expiry_threshold = self.silence_duration * 2;

        last_times.retain(|_, last_time| {
            let elapsed = now - *last_time;
            elapsed < expiry_threshold
        });

        info!("Cleaned up expired silence records, remaining: {}", last_times.len());
    }
}

/// 告警降噪器
pub struct AlertDeduplicator {
    // 指纹 -> 告警
    seen_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    // 去重窗口（秒）
    dedup_window: Duration,
}

impl AlertDeduplicator {
    pub fn new(dedup_window_secs: i64) -> Self {
        Self {
            seen_alerts: Arc::new(RwLock::new(HashMap::new())),
            dedup_window: Duration::seconds(dedup_window_secs),
        }
    }

    /// 检查告警是否重复
    pub async fn is_duplicate(&self, alert: &Alert) -> bool {
        let fingerprint = alert.fingerprint();
        let seen = self.seen_alerts.read().await;

        if let Some(existing) = seen.get(&fingerprint) {
            let elapsed = Utc::now() - existing.fired_at;
            if elapsed < self.dedup_window {
                debug!(
                    "Duplicate alert detected: {} (within {} seconds)",
                    alert.name,
                    elapsed.num_seconds()
                );
                return true;
            }
        }

        false
    }

    /// 记录告警
    pub async fn record_alert(&self, alert: Alert) {
        let fingerprint = alert.fingerprint();
        let mut seen = self.seen_alerts.write().await;
        seen.insert(fingerprint, alert);
    }

    /// 清理过期的记录
    pub async fn cleanup_expired(&self) {
        let mut seen = self.seen_alerts.write().await;
        let now = Utc::now();

        seen.retain(|_, alert| {
            let elapsed = now - alert.fired_at;
            elapsed < self.dedup_window * 2
        });

        info!("Cleaned up expired dedup records, remaining: {}", seen.len());
    }
}

/// 告警分组器
pub struct AlertGrouper {
    // 分组键 -> 告警列表
    groups: Arc<RwLock<HashMap<String, Vec<Alert>>>>,
}

impl AlertGrouper {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 按标签分组
    pub async fn group_by_label(&self, alerts: Vec<Alert>, label_key: &str) -> HashMap<String, Vec<Alert>> {
        let mut result = HashMap::new();

        for alert in alerts {
            let group_key = alert
                .labels
                .get(label_key)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());

            result
                .entry(group_key)
                .or_insert_with(Vec::new)
                .push(alert);
        }

        result
    }

    /// 按严重程度分组
    pub fn group_by_severity(alerts: Vec<Alert>) -> HashMap<AlertSeverity, Vec<Alert>> {
        let mut result = HashMap::new();

        for alert in alerts {
            result
                .entry(alert.severity)
                .or_insert_with(Vec::new)
                .push(alert);
        }

        result
    }
}

impl Default for AlertGrouper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_alert_aggregator() {
        let aggregator = AlertAggregator::new(60, 300);

        let alert = Alert::new(
            "test_alert".to_string(),
            AlertSeverity::Warning,
            "Test".to_string(),
            HashMap::new(),
        );

        // 第一次不应该被静默
        assert!(!aggregator.should_silence(&alert).await);

        // 记录告警
        aggregator.record_alert(&alert).await;

        // 第二次应该被静默
        assert!(aggregator.should_silence(&alert).await);
    }

    #[tokio::test]
    async fn test_alert_deduplicator() {
        let dedup = AlertDeduplicator::new(60);

        let alert = Alert::new(
            "test_alert".to_string(),
            AlertSeverity::Warning,
            "Test".to_string(),
            HashMap::new(),
        );

        // 第一次不是重复
        assert!(!dedup.is_duplicate(&alert).await);

        // 记录告警
        dedup.record_alert(alert.clone()).await;

        // 第二次是重复
        assert!(dedup.is_duplicate(&alert).await);
    }

    #[tokio::test]
    async fn test_alert_grouper() {
        let grouper = AlertGrouper::new();

        let mut labels1 = HashMap::new();
        labels1.insert("host".to_string(), "server1".to_string());

        let mut labels2 = HashMap::new();
        labels2.insert("host".to_string(), "server2".to_string());

        let alert1 = Alert::new(
            "alert1".to_string(),
            AlertSeverity::Warning,
            "Test1".to_string(),
            labels1,
        );

        let alert2 = Alert::new(
            "alert2".to_string(),
            AlertSeverity::Critical,
            "Test2".to_string(),
            labels2,
        );

        let alerts = vec![alert1, alert2];
        let grouped = grouper.group_by_label(alerts, "host").await;

        assert_eq!(grouped.len(), 2);
        assert!(grouped.contains_key("server1"));
        assert!(grouped.contains_key("server2"));
    }

    #[test]
    fn test_group_by_severity() {
        let alert1 = Alert::new(
            "alert1".to_string(),
            AlertSeverity::Warning,
            "Test1".to_string(),
            HashMap::new(),
        );

        let alert2 = Alert::new(
            "alert2".to_string(),
            AlertSeverity::Critical,
            "Test2".to_string(),
            HashMap::new(),
        );

        let alerts = vec![alert1, alert2];
        let grouped = AlertGrouper::group_by_severity(alerts);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get(&AlertSeverity::Warning).unwrap().len(), 1);
        assert_eq!(grouped.get(&AlertSeverity::Critical).unwrap().len(), 1);
    }
}
