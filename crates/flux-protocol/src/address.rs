use crate::types::ProtocolType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// 统一协议地址
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolAddress {
    /// 协议类型
    pub protocol: ProtocolType,
    
    /// 主机地址
    pub host: String,
    
    /// 端口
    pub port: u16,
    
    /// 数据点地址（协议相关）
    pub path: String,
    
    /// 额外参数
    pub params: HashMap<String, String>,
}

impl ProtocolAddress {
    /// 从 URI 解析
    /// 
    /// 示例:
    /// - modbus://192.168.1.100:502/holding/40001
    /// - coap://[::1]:5683/sensors/temperature
    /// - opcua://localhost:4840/ns=2;s=Machine.Temperature
    pub fn from_uri(uri: &str) -> anyhow::Result<Self> {
        let url = Url::parse(uri)?;
        
        let protocol = ProtocolType::from_str(url.scheme())
            .ok_or_else(|| anyhow::anyhow!("Unknown protocol: {}", url.scheme()))?;
        
        let host = url.host_str()
            .ok_or_else(|| anyhow::anyhow!("Missing host"))?
            .to_string();
        
        let port = url.port().unwrap_or_else(|| default_port(protocol));
        
        let path = url.path().trim_start_matches('/').to_string();
        
        let mut params = HashMap::new();
        for (key, value) in url.query_pairs() {
            params.insert(key.to_string(), value.to_string());
        }
        
        Ok(Self {
            protocol,
            host,
            port,
            path,
            params,
        })
    }
    
    /// 转换为 URI
    pub fn to_uri(&self) -> String {
        let mut uri = format!(
            "{}://{}:{}",
            self.protocol.as_str(),
            self.host,
            self.port
        );
        
        if !self.path.is_empty() {
            uri.push('/');
            uri.push_str(&self.path);
        }
        
        if !self.params.is_empty() {
            uri.push('?');
            let params: Vec<String> = self.params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            uri.push_str(&params.join("&"));
        }
        
        uri
    }
}

fn default_port(protocol: ProtocolType) -> u16 {
    match protocol {
        ProtocolType::Modbus => 502,
        ProtocolType::CoAP => 5683,
        ProtocolType::OpcUa => 4840,
        ProtocolType::Mqtt => 1883,
        ProtocolType::Http => 80,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modbus_uri() {
        let addr = ProtocolAddress::from_uri("modbus://192.168.1.100:502/holding/40001").unwrap();
        assert_eq!(addr.protocol, ProtocolType::Modbus);
        assert_eq!(addr.host, "192.168.1.100");
        assert_eq!(addr.port, 502);
        assert_eq!(addr.path, "holding/40001");
    }

    #[test]
    fn test_parse_coap_uri() {
        let addr = ProtocolAddress::from_uri("coap://localhost:5683/sensors/temperature").unwrap();
        assert_eq!(addr.protocol, ProtocolType::CoAP);
        assert_eq!(addr.host, "localhost");
        assert_eq!(addr.port, 5683);
        assert_eq!(addr.path, "sensors/temperature");
    }

    #[test]
    fn test_parse_opcua_uri() {
        let addr = ProtocolAddress::from_uri("opcua://localhost:4840/ns=2;s=Machine.Temperature").unwrap();
        assert_eq!(addr.protocol, ProtocolType::OpcUa);
        assert_eq!(addr.host, "localhost");
        assert_eq!(addr.port, 4840);
        assert_eq!(addr.path, "ns=2;s=Machine.Temperature");
    }

    #[test]
    fn test_to_uri() {
        let addr = ProtocolAddress {
            protocol: ProtocolType::Modbus,
            host: "192.168.1.100".to_string(),
            port: 502,
            path: "holding/40001".to_string(),
            params: HashMap::new(),
        };
        
        assert_eq!(addr.to_uri(), "modbus://192.168.1.100:502/holding/40001");
    }

    #[test]
    fn test_default_port() {
        let addr = ProtocolAddress::from_uri("modbus://192.168.1.100").unwrap();
        assert_eq!(addr.port, 502);
        
        let addr = ProtocolAddress::from_uri("coap://localhost").unwrap();
        assert_eq!(addr.port, 5683);
        
        let addr = ProtocolAddress::from_uri("opcua://localhost").unwrap();
        assert_eq!(addr.port, 4840);
    }
}
