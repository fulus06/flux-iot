use serde::{Deserialize, Serialize};

/// CoAP 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoapConfig {
    /// 服务器地址
    pub host: String,
    
    /// 端口
    pub port: u16,
    
    /// 超时时间（毫秒）
    pub timeout_ms: u64,
}

impl Default for CoapConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5683,
            timeout_ms: 5000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CoapConfig::default();
        assert_eq!(config.port, 5683);
    }
}
