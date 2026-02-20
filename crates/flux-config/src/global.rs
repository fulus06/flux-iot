use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::timeshift::TimeShiftGlobalConfig;
use flux_storage::DiskType;

/// 全局配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalConfig {
    pub system: SystemConfig,
    pub timeshift: TimeShiftGlobalConfig,
    pub storage: StorageGlobalConfig,
}

/// 系统配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SystemConfig {
    pub name: String,
    pub version: String,
}

/// 存储全局配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageGlobalConfig {
    pub root_dir: PathBuf,
    pub retention_days: u64,
    #[serde(default)]
    pub pools: Vec<StoragePoolConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoragePoolConfig {
    pub name: String,
    pub path: PathBuf,
    #[serde(default = "default_disk_type")]
    pub disk_type: DiskType,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default = "default_max_usage_percent")]
    pub max_usage_percent: f64,
}

fn default_disk_type() -> DiskType {
    DiskType::Unknown
}

fn default_priority() -> u8 {
    1
}

fn default_max_usage_percent() -> f64 {
    95.0
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            system: SystemConfig {
                name: "FLUX IOT Media Platform".to_string(),
                version: "1.0.0".to_string(),
            },
            timeshift: TimeShiftGlobalConfig::default(),
            storage: StorageGlobalConfig {
                root_dir: PathBuf::from("./data"),
                retention_days: 7,
                pools: Vec::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_global_config() {
        let config = GlobalConfig::default();
        assert_eq!(config.system.name, "FLUX IOT Media Platform");
        assert_eq!(config.storage.retention_days, 7);
    }
}
