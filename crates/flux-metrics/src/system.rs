use sysinfo::{System, Disks};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::debug;

use crate::collector::MetricsCollector;

/// 系统指标收集器
pub struct SystemMetricsCollector {
    system: System,
    disks: Disks,
    metrics: Arc<MetricsCollector>,
}

impl SystemMetricsCollector {
    pub fn new(metrics: Arc<MetricsCollector>) -> Self {
        Self {
            system: System::new_all(),
            disks: Disks::new_with_refreshed_list(),
            metrics,
        }
    }

    /// 更新系统指标
    pub fn update(&mut self) {
        // 刷新 CPU 信息
        self.system.refresh_cpu();
        
        // 刷新内存信息
        self.system.refresh_memory();
        
        // 刷新磁盘信息
        self.disks.refresh();

        // 获取全局 CPU 使用率
        let cpu_usage = self.system.global_cpu_info().cpu_usage() as f64;
        self.metrics.set_cpu_usage(cpu_usage);

        // 获取内存使用
        let memory_used = self.system.used_memory();
        self.metrics.set_memory_usage(memory_used);

        // 获取磁盘使用率
        for disk in self.disks.list() {
            let mount_point = disk.mount_point().to_string_lossy().to_string();
            let total = disk.total_space() as f64;
            let available = disk.available_space() as f64;
            
            if total > 0.0 {
                let usage = ((total - available) / total) * 100.0;
                self.metrics.set_disk_usage(&mount_point, usage);
                
                debug!(
                    mount_point = %mount_point,
                    usage = %usage,
                    total_gb = %(total / 1024.0 / 1024.0 / 1024.0),
                    available_gb = %(available / 1024.0 / 1024.0 / 1024.0),
                    "Disk metrics updated"
                );
            }
        }

        debug!(
            cpu = %cpu_usage,
            memory_mb = %(memory_used / 1024 / 1024),
            "System metrics updated"
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
