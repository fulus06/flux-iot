use serde::{Deserialize, Serialize};

/// 健康状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 警告
    Warning,
    /// 严重
    Critical,
    /// 失败
    Failed,
}

impl HealthStatus {
    /// 从使用率判断健康状态
    pub fn from_usage_percent(usage: f64) -> Self {
        if usage >= 95.0 {
            HealthStatus::Critical
        } else if usage >= 85.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }

    /// 是否健康
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// 是否需要告警
    pub fn needs_alert(&self) -> bool {
        matches!(self, HealthStatus::Warning | HealthStatus::Critical | HealthStatus::Failed)
    }
}

/// 健康检查器
pub struct HealthChecker {
    warning_threshold: f64,
    critical_threshold: f64,
}

impl HealthChecker {
    pub fn new(warning_threshold: f64, critical_threshold: f64) -> Self {
        Self {
            warning_threshold,
            critical_threshold,
        }
    }

    /// 检查磁盘健康状态
    pub fn check_disk_health(&self, usage_percent: f64) -> HealthStatus {
        if usage_percent >= self.critical_threshold {
            HealthStatus::Critical
        } else if usage_percent >= self.warning_threshold {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new(85.0, 95.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        assert_eq!(HealthStatus::from_usage_percent(50.0), HealthStatus::Healthy);
        assert_eq!(HealthStatus::from_usage_percent(90.0), HealthStatus::Warning);
        assert_eq!(HealthStatus::from_usage_percent(96.0), HealthStatus::Critical);
    }

    #[test]
    fn test_health_checker() {
        let checker = HealthChecker::default();

        assert_eq!(checker.check_disk_health(50.0), HealthStatus::Healthy);
        assert_eq!(checker.check_disk_health(90.0), HealthStatus::Warning);
        assert_eq!(checker.check_disk_health(96.0), HealthStatus::Critical);
    }
}
