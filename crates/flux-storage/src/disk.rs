use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sysinfo::Disks;

/// 磁盘类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiskType {
    SSD,
    HDD,
    NVMe,
    Unknown,
}

/// 磁盘信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: PathBuf,
    pub total_space: u64,
    pub available_space: u64,
    pub disk_type: DiskType,
    pub file_system: String,
}

impl DiskInfo {
    /// 使用率百分比
    pub fn usage_percent(&self) -> f64 {
        if self.total_space == 0 {
            return 0.0;
        }
        (self.total_space - self.available_space) as f64 / self.total_space as f64 * 100.0
    }

    /// 是否可用
    pub fn is_available(&self) -> bool {
        self.available_space > 0 && self.usage_percent() < 95.0
    }
}

/// 磁盘监控器
pub struct DiskMonitor {
    disks: Disks,
}

impl DiskMonitor {
    pub fn new() -> Self {
        Self {
            disks: Disks::new_with_refreshed_list(),
        }
    }

    /// 扫描所有磁盘
    pub fn scan_disks(&mut self) -> Result<Vec<DiskInfo>> {
        self.disks.refresh_list();

        let mut disk_infos = Vec::new();

        for disk in self.disks.list() {
            let info = DiskInfo {
                name: disk.name().to_str().unwrap_or("unknown").to_string(),
                mount_point: disk.mount_point().to_path_buf(),
                total_space: disk.total_space(),
                available_space: disk.available_space(),
                disk_type: Self::detect_disk_type(disk),
                file_system: disk.file_system().to_str().unwrap_or("unknown").to_string(),
            };

            disk_infos.push(info);
        }

        Ok(disk_infos)
    }

    /// 刷新磁盘信息
    pub fn refresh(&mut self) {
        self.disks.refresh();
    }

    /// 检测磁盘类型
    fn detect_disk_type(disk: &sysinfo::Disk) -> DiskType {
        let name = disk.name().to_str().unwrap_or("").to_lowercase();

        if name.contains("nvme") {
            DiskType::NVMe
        } else if name.contains("ssd") {
            DiskType::SSD
        } else if name.contains("hd") || name.contains("sd") {
            DiskType::HDD
        } else {
            // 尝试通过挂载点判断
            let mount = disk.mount_point().to_str().unwrap_or("").to_lowercase();
            if mount.contains("ssd") {
                DiskType::SSD
            } else {
                DiskType::Unknown
            }
        }
    }
}

impl Default for DiskMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_monitor() {
        let mut monitor = DiskMonitor::new();
        let disks = monitor.scan_disks().unwrap();
        assert!(!disks.is_empty());

        for disk in disks {
            println!("Disk: {} - {:?}", disk.name, disk.disk_type);
            println!("  Mount: {:?}", disk.mount_point);
            println!("  Total: {} GB", disk.total_space / 1024 / 1024 / 1024);
            println!("  Available: {} GB", disk.available_space / 1024 / 1024 / 1024);
            println!("  Usage: {:.1}%", disk.usage_percent());
        }
    }
}
