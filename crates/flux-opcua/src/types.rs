use serde::{Deserialize, Serialize};

/// OPC UA 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpcUaConfig {
    /// 服务器端点 URL
    pub endpoint_url: String,
    
    /// 安全策略
    pub security_policy: String,
    
    /// 安全模式
    pub security_mode: String,
    
    /// 用户名（可选）
    pub username: Option<String>,
    
    /// 密码（可选）
    pub password: Option<String>,
}

impl Default for OpcUaConfig {
    fn default() -> Self {
        Self {
            endpoint_url: "opc.tcp://localhost:4840".to_string(),
            security_policy: "None".to_string(),
            security_mode: "None".to_string(),
            username: None,
            password: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OpcUaConfig::default();
        assert_eq!(config.endpoint_url, "opc.tcp://localhost:4840");
    }
}
