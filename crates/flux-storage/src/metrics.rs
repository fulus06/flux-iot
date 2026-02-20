use serde::{Deserialize, Serialize};

/// 存储指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetrics {
    /// 总空间（字节）
    pub total_space: u64,
    
    /// 已用空间（字节）
    pub used_space: u64,
    
    /// 可用空间（字节）
    pub available_space: u64,
    
    /// 使用率（百分比）
    pub usage_percent: f64,
    
    /// 健康磁盘数
    pub healthy_disks: usize,
    
    /// 警告磁盘数
    pub warning_disks: usize,
    
    /// 严重磁盘数
    pub critical_disks: usize,
    
    /// 失败磁盘数
    pub failed_disks: usize,
    
    /// 总磁盘数
    pub total_disks: usize,
}

impl StorageMetrics {
    pub fn new() -> Self {
        Self {
            total_space: 0,
            used_space: 0,
            available_space: 0,
            usage_percent: 0.0,
            healthy_disks: 0,
            warning_disks: 0,
            critical_disks: 0,
            failed_disks: 0,
            total_disks: 0,
        }
    }

    /// 格式化空间大小
    pub fn format_space(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        if bytes >= TB {
            format!("{:.2} TB", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }
}

impl Default for StorageMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_space() {
        assert_eq!(StorageMetrics::format_space(1024), "1.00 KB");
        assert_eq!(StorageMetrics::format_space(1024 * 1024), "1.00 MB");
        assert_eq!(StorageMetrics::format_space(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(StorageMetrics::format_space(1024 * 1024 * 1024 * 1024), "1.00 TB");
    }
}
