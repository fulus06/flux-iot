use flux_protocol::{ProtocolAddress, ProtocolClient, ProtocolFactory, ProtocolType};
use tracing::{debug, info};

#[cfg(feature = "modbus")]
use flux_modbus::{ModbusAdapter, ModbusConfig};

#[cfg(feature = "coap")]
use flux_coap::{CoapAdapter, CoapConfig};

#[cfg(feature = "opcua")]
use flux_opcua::{OpcUaAdapter, OpcUaConfig};

/// 默认协议工厂实现
#[derive(Debug, Clone, Default)]
pub struct DefaultProtocolFactory;

impl DefaultProtocolFactory {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ProtocolFactory for DefaultProtocolFactory {
    /// 从 URI 创建协议客户端
    /// 
    /// 支持的 URI 格式:
    /// - Modbus: `modbus://192.168.1.100:502?slave_id=1&timeout_ms=5000`
    /// - CoAP: `coap://localhost:5683/sensors/temperature?timeout_ms=3000`
    /// - OPC UA: `opcua://localhost:4840?security_policy=None&username=admin`
    async fn from_uri(&self, uri: &str) -> anyhow::Result<Box<dyn ProtocolClient>> {
        debug!(uri = %uri, "Creating protocol client from URI");
        
        let address = ProtocolAddress::from_uri(uri)?;
        self.from_address(&address).await
    }
    
    /// 从地址创建协议客户端
    async fn from_address(&self, address: &ProtocolAddress) -> anyhow::Result<Box<dyn ProtocolClient>> {
        info!(
            protocol = ?address.protocol,
            host = %address.host,
            port = %address.port,
            "Creating protocol client"
        );
        
        match address.protocol {
            ProtocolType::Modbus => Self::create_modbus_client(address).await,
            ProtocolType::CoAP => Self::create_coap_client(address).await,
            ProtocolType::OpcUa => Self::create_opcua_client(address).await,
            _ => Err(anyhow::anyhow!("Unsupported protocol: {:?}", address.protocol)),
        }
    }
}

impl DefaultProtocolFactory {
    /// 创建 Modbus 客户端
    #[cfg(feature = "modbus")]
    async fn create_modbus_client(address: &ProtocolAddress) -> anyhow::Result<Box<dyn ProtocolClient>> {
        let slave_id = address.params
            .get("slave_id")
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(1);
        
        let timeout_ms = address.params
            .get("timeout_ms")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5000);
        
        let config = ModbusConfig {
            host: address.host.clone(),
            port: address.port,
            slave_id,
            timeout_ms,
        };
        
        debug!(config = ?config, "Creating Modbus adapter");
        Ok(Box::new(ModbusAdapter::new(config)))
    }
    
    #[cfg(not(feature = "modbus"))]
    async fn create_modbus_client(_address: &ProtocolAddress) -> anyhow::Result<Box<dyn ProtocolClient>> {
        Err(anyhow::anyhow!("Modbus support not enabled. Enable 'modbus' feature."))
    }
    
    /// 创建 CoAP 客户端
    #[cfg(feature = "coap")]
    async fn create_coap_client(address: &ProtocolAddress) -> anyhow::Result<Box<dyn ProtocolClient>> {
        let timeout_ms = address.params
            .get("timeout_ms")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5000);
        
        let config = CoapConfig {
            host: address.host.clone(),
            port: address.port,
            timeout_ms,
        };
        
        debug!(config = ?config, "Creating CoAP adapter");
        Ok(Box::new(CoapAdapter::new(config)))
    }
    
    #[cfg(not(feature = "coap"))]
    async fn create_coap_client(_address: &ProtocolAddress) -> anyhow::Result<Box<dyn ProtocolClient>> {
        Err(anyhow::anyhow!("CoAP support not enabled. Enable 'coap' feature."))
    }
    
    /// 创建 OPC UA 客户端
    #[cfg(feature = "opcua")]
    async fn create_opcua_client(address: &ProtocolAddress) -> anyhow::Result<Box<dyn ProtocolClient>> {
        let endpoint_url = format!("opc.tcp://{}:{}", address.host, address.port);
        
        let security_policy = address.params
            .get("security_policy")
            .cloned()
            .unwrap_or_else(|| "None".to_string());
        
        let security_mode = address.params
            .get("security_mode")
            .cloned()
            .unwrap_or_else(|| "None".to_string());
        
        let username = address.params.get("username").cloned();
        let password = address.params.get("password").cloned();
        
        let config = OpcUaConfig {
            endpoint_url,
            security_policy,
            security_mode,
            username,
            password,
        };
        
        debug!(config = ?config, "Creating OPC UA adapter");
        Ok(Box::new(OpcUaAdapter::new(config)))
    }
    
    #[cfg(not(feature = "opcua"))]
    async fn create_opcua_client(_address: &ProtocolAddress) -> anyhow::Result<Box<dyn ProtocolClient>> {
        Err(anyhow::anyhow!("OPC UA support not enabled. Enable 'opcua' feature."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_modbus_uri() {
        let uri = "modbus://192.168.1.100:502?slave_id=2&timeout_ms=3000";
        let address = ProtocolAddress::from_uri(uri).unwrap();
        
        assert_eq!(address.protocol, ProtocolType::Modbus);
        assert_eq!(address.host, "192.168.1.100");
        assert_eq!(address.port, 502);
        assert_eq!(address.params.get("slave_id"), Some(&"2".to_string()));
    }

    #[tokio::test]
    async fn test_parse_coap_uri() {
        let uri = "coap://localhost:5683/sensors/temp?timeout_ms=2000";
        let address = ProtocolAddress::from_uri(uri).unwrap();
        
        assert_eq!(address.protocol, ProtocolType::CoAP);
        assert_eq!(address.host, "localhost");
        assert_eq!(address.port, 5683);
        assert_eq!(address.path, "sensors/temp");
    }

    #[tokio::test]
    async fn test_parse_opcua_uri() {
        let uri = "opcua://localhost:4840?username=admin&password=secret";
        let address = ProtocolAddress::from_uri(uri).unwrap();
        
        assert_eq!(address.protocol, ProtocolType::OpcUa);
        assert_eq!(address.host, "localhost");
        assert_eq!(address.port, 4840);
        assert_eq!(address.params.get("username"), Some(&"admin".to_string()));
    }

    #[cfg(feature = "modbus")]
    #[tokio::test]
    async fn test_create_modbus_client() {
        let factory = DefaultProtocolFactory::new();
        let uri = "modbus://192.168.1.100:502?slave_id=1";
        let result = factory.from_uri(uri).await;
        
        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.protocol_type(), ProtocolType::Modbus);
    }

    #[cfg(feature = "coap")]
    #[tokio::test]
    async fn test_create_coap_client() {
        let factory = DefaultProtocolFactory::new();
        let uri = "coap://localhost:5683";
        let result = factory.from_uri(uri).await;
        
        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.protocol_type(), ProtocolType::CoAP);
    }

    #[cfg(feature = "opcua")]
    #[tokio::test]
    async fn test_create_opcua_client() {
        let factory = DefaultProtocolFactory::new();
        let uri = "opcua://localhost:4840";
        let result = factory.from_uri(uri).await;
        
        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.protocol_type(), ProtocolType::OpcUa);
    }

    #[tokio::test]
    async fn test_unsupported_protocol() {
        let factory = DefaultProtocolFactory::new();
        let uri = "mqtt://localhost:1883";
        let result = factory.from_uri(uri).await;
        
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Unsupported protocol"));
        }
    }
    
    #[tokio::test]
    async fn test_default_parameters() {
        let factory = DefaultProtocolFactory::new();
        
        #[cfg(feature = "modbus")]
        {
            let uri = "modbus://192.168.1.100";
            let result = factory.from_uri(uri).await;
            assert!(result.is_ok());
        }
        
        #[cfg(feature = "coap")]
        {
            let uri = "coap://localhost";
            let result = factory.from_uri(uri).await;
            assert!(result.is_ok());
        }
        
        #[cfg(feature = "opcua")]
        {
            let uri = "opcua://localhost";
            let result = factory.from_uri(uri).await;
            assert!(result.is_ok());
        }
    }
}
