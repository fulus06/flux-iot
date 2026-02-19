use serde::{Deserialize, Serialize};

/// 时移配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimeShiftConfig {
    /// 是否启用时移
    pub enabled: bool,
    
    /// 热缓存时长（秒）- 保留在内存中
    pub hot_cache_duration: u64,
    
    /// 冷存储时长（秒）- 保留在磁盘上
    pub cold_storage_duration: u64,
    
    /// 最大分片数
    pub max_segments: usize,
    
    /// 批量写入大小
    pub batch_write_size: usize,
    
    /// 批量写入间隔（秒）
    pub batch_write_interval: u64,
    
    /// LRU 缓存大小（MB）
    pub lru_cache_size_mb: usize,
}

impl Default for TimeShiftConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_cache_duration: 300,      // 5 分钟
            cold_storage_duration: 3600,  // 60 分钟
            max_segments: 600,
            batch_write_size: 10,
            batch_write_interval: 5,
            lru_cache_size_mb: 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TimeShiftConfig::default();
        assert!(config.enabled);
        assert_eq!(config.hot_cache_duration, 300);
        assert_eq!(config.cold_storage_duration, 3600);
    }
}
