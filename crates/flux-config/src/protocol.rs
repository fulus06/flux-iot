use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::global::StoragePoolConfig;
use crate::timeshift::TimeShiftProtocolConfig;

/// 协议配置（泛型，支持不同协议的服务器配置）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtocolConfig<T> {
    pub server: T,
    #[serde(default)]
    pub storage: Option<ProtocolStorageConfig>,
    #[serde(default)]
    pub timeshift: Option<TimeShiftProtocolConfig>,
}

/// 协议存储配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtocolStorageConfig {
    pub storage_dir: PathBuf,
    pub keyframe_dir: Option<PathBuf>,
    #[serde(default)]
    pub pools: Option<Vec<StoragePoolConfig>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Deserialize, Serialize)]
    struct TestServerConfig {
        bind: String,
    }

    #[test]
    fn test_protocol_config() {
        let config = ProtocolConfig {
            server: TestServerConfig {
                bind: "0.0.0.0:1935".to_string(),
            },
            storage: Some(ProtocolStorageConfig {
                storage_dir: PathBuf::from("./data/test"),
                keyframe_dir: None,
                pools: None,
            }),
            timeshift: None,
        };
        
        assert_eq!(config.server.bind, "0.0.0.0:1935");
        assert!(config.storage.is_some());
        assert!(config.timeshift.is_none());
    }
}
