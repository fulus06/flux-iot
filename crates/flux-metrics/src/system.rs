use sysinfo::System;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::debug;

use crate::collector::MetricsCollector;

/// 系统指标收集器
pub struct SystemMetricsCollector {
    system: System,
    metrics: Arc<MetricsCollector>,
}

impl SystemMetricsCollector {
    pub fn new(metrics: Arc<MetricsCollector>) -> Self {
        Self {
            system: System::new_all(),
            metrics,
        }
    }

    /// 更新系统指标
    pub fn update(&mut self) {
        self.system.refresh_all();

        // CPU 使用率（简化实现，设置为 0）
        let cpu_usage = 0.0;
        self.metrics.set_cpu_usage(cpu_usage);

        // 内存使用
        let memory_used = self.system.used_memory();
        self.metrics.set_memory_usage(memory_used);

        // 磁盘使用（简化实现）
        self.metrics.set_disk_usage("/", 0.0);

        debug!(
            "System metrics updated: CPU={:.2}%, Memory={}MB",
            cpu_usage * 100.0,
            memory_used / 1024 / 1024
        );
    }

    /// 启动定期收集
    pub async fn start_periodic_collection(mut self, interval_secs: u64) {
        let mut ticker = interval(Duration::from_secs(interval_secs));

        tokio::spawn(async move {
            loop {
                ticker.tick().await;
                self.update();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_metrics_collector() {
        let metrics = Arc::new(MetricsCollector::new().unwrap());
        let mut collector = SystemMetricsCollector::new(metrics.clone());

        collector.update();

        let exported = metrics.export().unwrap();
        assert!(exported.contains("cpu_usage_ratio"));
        assert!(exported.contains("memory_usage_bytes"));
    }
}
