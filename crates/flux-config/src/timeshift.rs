use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 时移全局配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimeShiftGlobalConfig {
    pub enabled: bool,
    pub hot_cache_duration: u64,
    pub cold_storage_duration: u64,
    pub max_segments: usize,
    pub storage_root: PathBuf,
    pub batch_write_size: usize,
    pub batch_write_interval: u64,
    pub lru_cache_size_mb: usize,
}

impl Default for TimeShiftGlobalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_cache_duration: 300,      // 5 分钟
            cold_storage_duration: 3600,  // 60 分钟
            max_segments: 600,
            storage_root: PathBuf::from("./data/timeshift"),
            batch_write_size: 10,
            batch_write_interval: 5,
            lru_cache_size_mb: 500,
        }
    }
}

/// 协议时移配置（可覆盖全局）
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TimeShiftProtocolConfig {
    pub enabled: Option<bool>,
    pub hot_cache_duration: Option<u64>,
    pub cold_storage_duration: Option<u64>,
    pub max_segments: Option<usize>,
}

impl TimeShiftProtocolConfig {
    /// 合并全局配置和协议配置
    pub fn merge_with_global(&self, global: &TimeShiftGlobalConfig) -> TimeShiftMergedConfig {
        TimeShiftMergedConfig {
            enabled: self.enabled.unwrap_or(global.enabled),
            hot_cache_duration: self.hot_cache_duration.unwrap_or(global.hot_cache_duration),
            cold_storage_duration: self.cold_storage_duration.unwrap_or(global.cold_storage_duration),
            max_segments: self.max_segments.unwrap_or(global.max_segments),
            storage_root: global.storage_root.clone(),
            batch_write_size: global.batch_write_size,
            batch_write_interval: global.batch_write_interval,
            lru_cache_size_mb: global.lru_cache_size_mb,
        }
    }
}

/// 合并后的时移配置
#[derive(Debug, Clone)]
pub struct TimeShiftMergedConfig {
    pub enabled: bool,
    pub hot_cache_duration: u64,
    pub cold_storage_duration: u64,
    pub max_segments: usize,
    pub storage_root: PathBuf,
    pub batch_write_size: usize,
    pub batch_write_interval: u64,
    pub lru_cache_size_mb: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_with_global() {
        let global = TimeShiftGlobalConfig::default();
        let protocol = TimeShiftProtocolConfig {
            enabled: Some(true),
            hot_cache_duration: Some(600),
            cold_storage_duration: None,
            max_segments: None,
        };
        
        let merged = protocol.merge_with_global(&global);
        
        assert_eq!(merged.enabled, true);
        assert_eq!(merged.hot_cache_duration, 600);  // 使用协议配置
        assert_eq!(merged.cold_storage_duration, 3600);  // 使用全局配置
        assert_eq!(merged.max_segments, 600);  // 使用全局配置
    }

    #[test]
    fn test_merge_all_from_global() {
        let global = TimeShiftGlobalConfig::default();
        let protocol = TimeShiftProtocolConfig::default();
        
        let merged = protocol.merge_with_global(&global);
        
        assert_eq!(merged.enabled, global.enabled);
        assert_eq!(merged.hot_cache_duration, global.hot_cache_duration);
    }
}
