use serde::{Deserialize, Serialize};

/// Modbus 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusConfig {
    /// 服务器地址
    pub host: String,
    
    /// 端口
    pub port: u16,
    
    /// 从站 ID
    pub slave_id: u8,
    
    /// 连接超时（毫秒）
    pub timeout_ms: u64,
}

impl Default for ModbusConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 502,
            slave_id: 1,
            timeout_ms: 5000,
        }
    }
}

/// 寄存器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterType {
    /// 保持寄存器（可读写，地址 40001-49999）
    Holding,
    
    /// 输入寄存器（只读，地址 30001-39999）
    Input,
    
    /// 线圈（可读写，地址 00001-09999）
    Coil,
    
    /// 离散输入（只读，地址 10001-19999）
    DiscreteInput,
}

impl RegisterType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "holding" => Some(Self::Holding),
            "input" => Some(Self::Input),
            "coil" => Some(Self::Coil),
            "discrete" | "discrete_input" => Some(Self::DiscreteInput),
            _ => None,
        }
    }
}

/// 解析 Modbus 地址
/// 格式: "holding/40001" 或 "input/30001"
pub fn parse_modbus_address(address: &str) -> anyhow::Result<(RegisterType, u16)> {
    let parts: Vec<&str> = address.split('/').collect();
    
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid Modbus address format: {}", address));
    }
    
    let register_type = RegisterType::from_str(parts[0])
        .ok_or_else(|| anyhow::anyhow!("Unknown register type: {}", parts[0]))?;
    
    let addr: u16 = parts[1].parse()
        .map_err(|_| anyhow::anyhow!("Invalid address number: {}", parts[1]))?;
    
    Ok((register_type, addr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modbus_address() {
        let (reg_type, addr) = parse_modbus_address("holding/40001").unwrap();
        assert_eq!(reg_type, RegisterType::Holding);
        assert_eq!(addr, 40001);

        let (reg_type, addr) = parse_modbus_address("input/30001").unwrap();
        assert_eq!(reg_type, RegisterType::Input);
        assert_eq!(addr, 30001);
    }
}
