use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolType {
    Modbus,
    CoAP,
    OpcUa,
    Mqtt,
    Http,
}

impl ProtocolType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "modbus" => Some(Self::Modbus),
            "coap" => Some(Self::CoAP),
            "opcua" | "opc-ua" | "opc.ua" => Some(Self::OpcUa),
            "mqtt" => Some(Self::Mqtt),
            "http" | "https" => Some(Self::Http),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Modbus => "modbus",
            Self::CoAP => "coap",
            Self::OpcUa => "opcua",
            Self::Mqtt => "mqtt",
            Self::Http => "http",
        }
    }
}

/// 协议配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    pub protocol_type: ProtocolType,
    pub host: String,
    pub port: u16,
    pub params: HashMap<String, String>,
}

/// 订阅句柄
#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    pub id: String,
}

impl SubscriptionHandle {
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self { id: id.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_type_from_str() {
        assert_eq!(ProtocolType::from_str("modbus"), Some(ProtocolType::Modbus));
        assert_eq!(ProtocolType::from_str("MODBUS"), Some(ProtocolType::Modbus));
        assert_eq!(ProtocolType::from_str("coap"), Some(ProtocolType::CoAP));
        assert_eq!(ProtocolType::from_str("opcua"), Some(ProtocolType::OpcUa));
        assert_eq!(ProtocolType::from_str("opc-ua"), Some(ProtocolType::OpcUa));
    }

    #[test]
    fn test_protocol_type_as_str() {
        assert_eq!(ProtocolType::Modbus.as_str(), "modbus");
        assert_eq!(ProtocolType::CoAP.as_str(), "coap");
        assert_eq!(ProtocolType::OpcUa.as_str(), "opcua");
    }
}
