use crate::client::ModbusClient;
use crate::types::{parse_modbus_address, ModbusConfig, RegisterType};
use async_trait::async_trait;
use flux_protocol::{ProtocolClient, ProtocolType, SubscriptionHandle};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

/// Modbus 协议适配器
pub struct ModbusAdapter {
    client: Arc<Mutex<ModbusClient>>,
    connected: Arc<Mutex<bool>>,
}

impl ModbusAdapter {
    /// 创建新的 Modbus 适配器
    pub fn new(config: ModbusConfig) -> Self {
        Self {
            client: Arc::new(Mutex::new(ModbusClient::new(config))),
            connected: Arc::new(Mutex::new(false)),
        }
    }
}

#[async_trait]
impl ProtocolClient for ModbusAdapter {
    async fn connect(&mut self) -> anyhow::Result<()> {
        let mut client = self.client.lock().await;
        client.connect().await?;
        *self.connected.lock().await = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        let mut client = self.client.lock().await;
        client.disconnect().await?;
        *self.connected.lock().await = false;
        Ok(())
    }

    async fn read(&self, address: &str) -> anyhow::Result<Value> {
        let mut client = self.client.lock().await;
        let (register_type, addr) = parse_modbus_address(address)?;
        
        match register_type {
            RegisterType::Holding => {
                let value = client.read_holding_register(addr).await?;
                Ok(serde_json::json!(value))
            }
            RegisterType::Input => {
                let values = client.read_input_registers(addr, 1).await?;
                Ok(serde_json::json!(values[0]))
            }
            RegisterType::Coil => {
                let values = client.read_coils(addr, 1).await?;
                Ok(serde_json::json!(values[0]))
            }
            RegisterType::DiscreteInput => {
                let values = client.read_discrete_inputs(addr, 1).await?;
                Ok(serde_json::json!(values[0]))
            }
        }
    }

    async fn read_multiple(&self, addresses: &[String]) -> anyhow::Result<Vec<Value>> {
        let mut results = Vec::new();
        for addr in addresses {
            let value = self.read(addr).await?;
            results.push(value);
        }
        Ok(results)
    }

    async fn write(&self, address: &str, value: Value) -> anyhow::Result<()> {
        let mut client = self.client.lock().await;
        let (register_type, addr) = parse_modbus_address(address)?;
        
        match register_type {
            RegisterType::Holding => {
                let val = value.as_u64()
                    .ok_or_else(|| anyhow::anyhow!("Invalid value type"))?;
                client.write_holding_register(addr, val as u16).await?;
            }
            RegisterType::Coil => {
                let val = value.as_bool()
                    .ok_or_else(|| anyhow::anyhow!("Invalid value type"))?;
                client.write_coil(addr, val).await?;
            }
            _ => {
                return Err(anyhow::anyhow!("Cannot write to {:?} registers", register_type));
            }
        }
        
        Ok(())
    }

    async fn write_multiple(&self, data: &[(String, Value)]) -> anyhow::Result<()> {
        for (addr, value) in data {
            self.write(addr, value.clone()).await?;
        }
        Ok(())
    }

    async fn subscribe(
        &self,
        _address: &str,
        _callback: Box<dyn Fn(Value) + Send + Sync>,
    ) -> anyhow::Result<SubscriptionHandle> {
        // Modbus 不支持订阅，需要轮询
        warn!("Modbus does not support subscriptions, use polling instead");
        Err(anyhow::anyhow!("Modbus does not support subscriptions"))
    }

    async fn unsubscribe(&self, _handle: SubscriptionHandle) -> anyhow::Result<()> {
        Ok(())
    }

    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Modbus
    }

    fn is_connected(&self) -> bool {
        // 由于是同步方法，无法 await，返回一个保守的值
        // 实际应用中可以考虑使用 RwLock 或其他方案
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modbus_adapter_creation() {
        let config = ModbusConfig::default();
        let adapter = ModbusAdapter::new(config);
        assert_eq!(adapter.protocol_type(), ProtocolType::Modbus);
        assert!(!adapter.is_connected());
    }
}
