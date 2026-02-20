use anyhow::{anyhow, Result};
use config::{Config, File, FileFormat};
use std::path::{Path, PathBuf};

use crate::{GlobalConfig, TimeShiftMergedConfig};
use flux_storage::{DiskType, PoolConfig};

/// 配置加载器
pub struct ConfigLoader {
    config_dir: PathBuf,
}

impl ConfigLoader {
    /// 创建配置加载器
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
        }
    }

    /// 加载全局配置
    pub fn load_global(&self) -> Result<GlobalConfig> {
        let config_path = self.config_dir.join("global.toml");
        
        if !config_path.exists() {
            // 如果配置文件不存在，返回默认配置
            return Ok(GlobalConfig::default());
        }
        
        let config = Config::builder()
            .add_source(File::new(
                config_path.to_str().ok_or_else(|| anyhow!("Invalid config path"))?,
                FileFormat::Toml,
            ))
            .build()?;
        
        Ok(config.try_deserialize()?)
    }

    /// 加载协议配置
    pub fn load_protocol<T>(&self, protocol_name: &str) -> Result<crate::ProtocolConfig<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let config_path = self.config_dir
            .join("protocols")
            .join(format!("{}.toml", protocol_name));
        
        if !config_path.exists() {
            return Err(anyhow!("Protocol config not found: {}", protocol_name));
        }
        
        let config = Config::builder()
            .add_source(File::new(
                config_path.to_str().ok_or_else(|| anyhow!("Invalid config path"))?,
                FileFormat::Toml,
            ))
            .build()?;
        
        Ok(config.try_deserialize()?)
    }

    /// 加载并合并时移配置
    pub fn load_timeshift_config(&self, protocol_name: &str) -> Result<TimeShiftMergedConfig> {
        // 加载全局配置
        let global = self.load_global()?;
        
        // 尝试加载协议配置
        let protocol_config: Result<crate::ProtocolConfig<serde_json::Value>> = 
            self.load_protocol(protocol_name);
        
        // 合并配置
        if let Ok(protocol_cfg) = protocol_config {
            if let Some(protocol_timeshift) = protocol_cfg.timeshift {
                return Ok(protocol_timeshift.merge_with_global(&global.timeshift));
            }
        }
        
        // 使用全局配置
        Ok(crate::timeshift::TimeShiftProtocolConfig::default()
            .merge_with_global(&global.timeshift))
    }

    /// 加载存储池配置（多池）
    ///
    /// - 如果 config 目录下既没有 global.toml，也没有对应协议的配置文件，则返回 Ok(None)
    /// - 如果存在配置但未配置 pools，则使用 global.storage.root_dir 作为默认单池
    pub fn load_storage_pools(&self, protocol_name: &str) -> Result<Option<Vec<PoolConfig>>> {
        let global_path = self.config_dir.join("global.toml");
        let protocol_path = self
            .config_dir
            .join("protocols")
            .join(format!("{}.toml", protocol_name));

        if !global_path.exists() && !protocol_path.exists() {
            return Ok(None);
        }

        let global = self.load_global()?;

        // 优先协议覆盖
        let mut pools_cfg: Option<Vec<crate::StoragePoolConfig>> = None;
        if protocol_path.exists() {
            if let Ok(protocol_cfg) = self.load_protocol::<serde_json::Value>(protocol_name) {
                if let Some(storage_cfg) = protocol_cfg.storage {
                    if let Some(pools) = storage_cfg.pools {
                        if !pools.is_empty() {
                            pools_cfg = Some(pools);
                        }
                    }
                }
            }
        }

        // 使用全局 pools
        let pools_cfg = pools_cfg.unwrap_or_else(|| {
            if !global.storage.pools.is_empty() {
                global.storage.pools.clone()
            } else {
                vec![crate::StoragePoolConfig {
                    name: "default".to_string(),
                    path: global.storage.root_dir.clone(),
                    disk_type: DiskType::Unknown,
                    priority: 1,
                    max_usage_percent: 95.0,
                }]
            }
        });

        let pools = pools_cfg
            .into_iter()
            .map(|p| PoolConfig {
                name: p.name,
                path: p.path,
                disk_type: p.disk_type,
                priority: p.priority,
                max_usage_percent: p.max_usage_percent,
            })
            .collect::<Vec<_>>();

        Ok(Some(pools))
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        let global = self.load_global()?;
        
        // 验证时移配置
        if global.timeshift.hot_cache_duration > global.timeshift.cold_storage_duration {
            return Err(anyhow!(
                "hot_cache_duration ({}) cannot be greater than cold_storage_duration ({})",
                global.timeshift.hot_cache_duration,
                global.timeshift.cold_storage_duration
            ));
        }
        
        if global.timeshift.max_segments == 0 {
            return Err(anyhow!("max_segments must be greater than 0"));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_load_default_global_config() {
        let temp_dir = tempdir().unwrap();
        let loader = ConfigLoader::new(temp_dir.path());
        
        let config = loader.load_global().unwrap();
        assert_eq!(config.system.name, "FLUX IOT Media Platform");
    }

    #[test]
    fn test_load_global_config_from_file() {
        let temp_dir = tempdir().unwrap();
        let config_content = r#"
[system]
name = "Test Platform"
version = "2.0.0"

[timeshift]
enabled = true
hot_cache_duration = 600
cold_storage_duration = 7200
max_segments = 1200
storage_root = "./test/timeshift"
batch_write_size = 20
batch_write_interval = 10
lru_cache_size_mb = 1000

[storage]
root_dir = "./test/data"
retention_days = 14
"#;
        
        fs::write(temp_dir.path().join("global.toml"), config_content).unwrap();
        
        let loader = ConfigLoader::new(temp_dir.path());
        let config = loader.load_global().unwrap();
        
        assert_eq!(config.system.name, "Test Platform");
        assert_eq!(config.timeshift.hot_cache_duration, 600);
        assert_eq!(config.storage.retention_days, 14);
    }

    #[test]
    fn test_validate_config() {
        let temp_dir = tempdir().unwrap();
        let loader = ConfigLoader::new(temp_dir.path());
        
        assert!(loader.validate().is_ok());
    }
}
