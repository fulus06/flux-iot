use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 录像全局配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordingConfig {
    /// 是否启用录像
    pub enabled: bool,
    
    /// 保留天数
    pub retention_days: u64,
    
    /// 分片配置
    pub segment: RecordingSegmentConfig,
    
    /// 压缩配置
    pub compression: RecordingCompressionConfig,
    
    /// 质量配置
    pub quality: RecordingQualityConfig,
    
    /// 转换配置
    pub conversion: RecordingConversionConfig,
    
    /// 索引配置
    pub index: RecordingIndexConfig,
    
    /// 存储配置
    pub storage: RecordingStorageConfig,
}

/// 分片配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordingSegmentConfig {
    /// 分片策略
    pub strategy: SegmentStrategy,
    
    /// 最小时长（秒）
    pub min_duration: u64,
    
    /// 最大时长（秒）
    pub max_duration: u64,
    
    /// 目标文件大小（MB）
    pub target_size_mb: u64,
}

/// 分片策略
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SegmentStrategy {
    /// 固定时长
    Fixed,
    
    /// 固定大小
    Size,
    
    /// 自适应（推荐）
    Adaptive,
}

/// 压缩配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordingCompressionConfig {
    /// 实时压缩算法
    pub realtime: CompressionAlgorithm,
    
    /// 归档压缩算法
    pub archive: CompressionAlgorithm,
    
    /// 长期归档压缩算法
    pub longterm: CompressionAlgorithm,
    
    /// 多少小时后应用归档压缩
    pub apply_archive_after_hours: u64,
    
    /// 多少天后应用长期压缩
    pub apply_longterm_after_days: u64,
}

/// 压缩算法
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgorithm {
    /// 不压缩
    None,
    
    /// LZ4（快速）
    Lz4,
    
    /// Zstd（平衡）
    Zstd,
    
    /// Brotli（高压缩率）
    Brotli,
    
    /// LZMA（极限压缩）
    Lzma,
}

/// 质量配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordingQualityConfig {
    /// 实时录像质量
    pub realtime: Quality,
    
    /// 归档质量
    pub archive: Quality,
    
    /// 多少小时后降级
    pub downgrade_after_hours: u64,
}

/// 视频质量
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// 原始质量
    Original,
    
    /// 高质量 (1080p)
    High,
    
    /// 中等质量 (720p)
    Medium,
    
    /// 低质量 (480p)
    Low,
}

/// 转换配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordingConversionConfig {
    /// 是否启用自动转换
    pub enabled: bool,
    
    /// 触发时间（小时）
    pub trigger_after_hours: u64,
    
    /// 目标质量
    pub target_quality: Quality,
    
    /// 是否合并文件
    pub merge_files: bool,
    
    /// 合并目标时长（秒）
    pub merge_duration: u64,
    
    /// 转换并发数
    pub concurrency: usize,
}

/// 索引配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordingIndexConfig {
    /// 索引引擎
    pub engine: IndexEngine,
    
    /// 数据库路径
    pub db_path: PathBuf,
}

/// 索引引擎
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexEngine {
    /// JSON 文件
    Json,
    
    /// SQLite 数据库（推荐）
    Sqlite,
    
    /// 自定义二进制索引
    Binary,
}

/// 存储配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordingStorageConfig {
    /// 实时录像路径（SSD）
    pub realtime_path: PathBuf,
    
    /// 归档路径（HDD）
    pub archive_path: PathBuf,
    
    /// 长期归档路径（HDD）
    pub longterm_path: Option<PathBuf>,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 7,
            segment: RecordingSegmentConfig::default(),
            compression: RecordingCompressionConfig::default(),
            quality: RecordingQualityConfig::default(),
            conversion: RecordingConversionConfig::default(),
            index: RecordingIndexConfig::default(),
            storage: RecordingStorageConfig::default(),
        }
    }
}

impl Default for RecordingSegmentConfig {
    fn default() -> Self {
        Self {
            strategy: SegmentStrategy::Adaptive,
            min_duration: 60,      // 1 分钟
            max_duration: 300,     // 5 分钟
            target_size_mb: 75,    // 75 MB
        }
    }
}

impl Default for RecordingCompressionConfig {
    fn default() -> Self {
        Self {
            realtime: CompressionAlgorithm::Lz4,
            archive: CompressionAlgorithm::Zstd,
            longterm: CompressionAlgorithm::Brotli,
            apply_archive_after_hours: 24,
            apply_longterm_after_days: 7,
        }
    }
}

impl Default for RecordingQualityConfig {
    fn default() -> Self {
        Self {
            realtime: Quality::High,
            archive: Quality::Medium,
            downgrade_after_hours: 24,
        }
    }
}

impl Default for RecordingConversionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_after_hours: 24,
            target_quality: Quality::Medium,
            merge_files: true,
            merge_duration: 600,  // 10 分钟
            concurrency: 4,
        }
    }
}

impl Default for RecordingIndexConfig {
    fn default() -> Self {
        Self {
            engine: IndexEngine::Sqlite,
            db_path: PathBuf::from("./data/recordings.db"),
        }
    }
}

impl Default for RecordingStorageConfig {
    fn default() -> Self {
        Self {
            realtime_path: PathBuf::from("./data/recordings/realtime"),
            archive_path: PathBuf::from("./data/recordings/archive"),
            longterm_path: Some(PathBuf::from("./data/recordings/longterm")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_recording_config() {
        let config = RecordingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.retention_days, 7);
        assert_eq!(config.segment.min_duration, 60);
        assert_eq!(config.segment.max_duration, 300);
    }

    #[test]
    fn test_segment_strategy() {
        let config = RecordingSegmentConfig::default();
        assert!(matches!(config.strategy, SegmentStrategy::Adaptive));
    }

    #[test]
    fn test_compression_config() {
        let config = RecordingCompressionConfig::default();
        assert!(matches!(config.realtime, CompressionAlgorithm::Lz4));
        assert!(matches!(config.archive, CompressionAlgorithm::Zstd));
        assert!(matches!(config.longterm, CompressionAlgorithm::Brotli));
    }
}
