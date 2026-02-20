use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 告警级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// 告警状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertState {
    Firing,
    Resolved,
}

/// 告警实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub name: String,
    pub severity: AlertSeverity,
    pub state: AlertState,
    pub message: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub fired_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl Alert {
    pub fn new(
        name: String,
        severity: AlertSeverity,
        message: String,
        labels: HashMap<String, String>,
    ) -> Self {
        let id = format!("{}_{}", name, Utc::now().timestamp_millis());
        Self {
            id,
            name,
            severity,
            state: AlertState::Firing,
            message,
            labels,
            annotations: HashMap::new(),
            fired_at: Utc::now(),
            resolved_at: None,
        }
    }

    pub fn with_annotation(mut self, key: String, value: String) -> Self {
        self.annotations.insert(key, value);
        self
    }

    pub fn resolve(&mut self) {
        self.state = AlertState::Resolved;
        self.resolved_at = Some(Utc::now());
    }

    pub fn fingerprint(&self) -> String {
        let mut labels_str = self
            .labels
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>();
        labels_str.sort();
        format!("{}:{}", self.name, labels_str.join(","))
    }
}

/// 告警规则
pub trait AlertRule: Send + Sync {
    fn name(&self) -> &str;
    fn severity(&self) -> AlertSeverity;
    fn evaluate(&self, value: f64) -> bool;
    fn message(&self, value: f64) -> String;
    fn labels(&self) -> HashMap<String, String>;
}

/// 阈值告警规则
pub struct ThresholdRule {
    name: String,
    severity: AlertSeverity,
    threshold: f64,
    comparison: Comparison,
    labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
pub enum Comparison {
    GreaterThan,
    LessThan,
    Equal,
}

impl ThresholdRule {
    pub fn new(
        name: String,
        severity: AlertSeverity,
        threshold: f64,
        comparison: Comparison,
    ) -> Self {
        Self {
            name,
            severity,
            threshold,
            comparison,
            labels: HashMap::new(),
        }
    }

    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }
}

impl AlertRule for ThresholdRule {
    fn name(&self) -> &str {
        &self.name
    }

    fn severity(&self) -> AlertSeverity {
        self.severity
    }

    fn evaluate(&self, value: f64) -> bool {
        match self.comparison {
            Comparison::GreaterThan => value > self.threshold,
            Comparison::LessThan => value < self.threshold,
            Comparison::Equal => (value - self.threshold).abs() < f64::EPSILON,
        }
    }

    fn message(&self, value: f64) -> String {
        format!(
            "{}: value={:.2}, threshold={:.2}",
            self.name, value, self.threshold
        )
    }

    fn labels(&self) -> HashMap<String, String> {
        self.labels.clone()
    }
}

/// 告警规则引擎
pub struct AlertEngine {
    rules: Vec<Box<dyn AlertRule>>,
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    alert_history: Arc<RwLock<Vec<Alert>>>,
    max_history: usize,
}

impl AlertEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(Vec::new())),
            max_history: 1000,
        }
    }

    pub fn add_rule(&mut self, rule: Box<dyn AlertRule>) {
        info!("Adding alert rule: {}", rule.name());
        self.rules.push(rule);
    }

    pub async fn evaluate(&self, metric_name: &str, value: f64) -> Vec<Alert> {
        let mut new_alerts = Vec::new();

        for rule in &self.rules {
            if rule.name() == metric_name {
                if rule.evaluate(value) {
                    let mut labels = rule.labels();
                    labels.insert("metric".to_string(), metric_name.to_string());

                    let alert = Alert::new(
                        rule.name().to_string(),
                        rule.severity(),
                        rule.message(value),
                        labels,
                    );

                    let fingerprint = alert.fingerprint();
                    let mut active = self.active_alerts.write().await;

                    if !active.contains_key(&fingerprint) {
                        info!("Alert fired: {} - {}", alert.name, alert.message);
                        active.insert(fingerprint.clone(), alert.clone());
                        new_alerts.push(alert);
                    }
                } else {
                    // 检查是否需要解决告警
                    let mut labels = rule.labels();
                    labels.insert("metric".to_string(), metric_name.to_string());

                    let temp_alert = Alert::new(
                        rule.name().to_string(),
                        rule.severity(),
                        String::new(),
                        labels,
                    );
                    let fingerprint = temp_alert.fingerprint();

                    let mut active = self.active_alerts.write().await;
                    if let Some(mut alert) = active.remove(&fingerprint) {
                        alert.resolve();
                        info!("Alert resolved: {}", alert.name);

                        let mut history = self.alert_history.write().await;
                        history.push(alert);

                        if history.len() > self.max_history {
                            history.remove(0);
                        }
                    }
                }
            }
        }

        new_alerts
    }

    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let active = self.active_alerts.read().await;
        active.values().cloned().collect()
    }

    pub async fn get_alert_history(&self, limit: usize) -> Vec<Alert> {
        let history = self.alert_history.read().await;
        let start = if history.len() > limit {
            history.len() - limit
        } else {
            0
        };
        history[start..].to_vec()
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for AlertEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_rule() {
        let rule = ThresholdRule::new(
            "high_cpu".to_string(),
            AlertSeverity::Warning,
            0.8,
            Comparison::GreaterThan,
        );

        assert!(rule.evaluate(0.9));
        assert!(!rule.evaluate(0.7));
    }

    #[tokio::test]
    async fn test_alert_engine() {
        let mut engine = AlertEngine::new();

        let rule = Box::new(ThresholdRule::new(
            "high_cpu".to_string(),
            AlertSeverity::Warning,
            0.8,
            Comparison::GreaterThan,
        ));

        engine.add_rule(rule);

        // 触发告警
        let alerts = engine.evaluate("high_cpu", 0.9).await;
        assert_eq!(alerts.len(), 1);

        // 再次评估，不应该重复触发
        let alerts = engine.evaluate("high_cpu", 0.9).await;
        assert_eq!(alerts.len(), 0);

        // 解决告警
        let alerts = engine.evaluate("high_cpu", 0.5).await;
        assert_eq!(alerts.len(), 0);

        let active = engine.get_active_alerts().await;
        assert_eq!(active.len(), 0);
    }

    #[test]
    fn test_alert_fingerprint() {
        let mut labels = HashMap::new();
        labels.insert("host".to_string(), "server1".to_string());
        labels.insert("service".to_string(), "api".to_string());

        let alert1 = Alert::new(
            "high_cpu".to_string(),
            AlertSeverity::Warning,
            "CPU high".to_string(),
            labels.clone(),
        );

        let alert2 = Alert::new(
            "high_cpu".to_string(),
            AlertSeverity::Warning,
            "CPU high".to_string(),
            labels,
        );

        assert_eq!(alert1.fingerprint(), alert2.fingerprint());
    }
}
