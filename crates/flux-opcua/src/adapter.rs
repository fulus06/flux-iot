use crate::client::OpcUaClient;
use crate::types::OpcUaConfig;
use async_trait::async_trait;
use flux_protocol::{ProtocolClient, ProtocolType, SubscriptionHandle};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

/// OPC UA 协议适配器
pub struct OpcUaAdapter {
    client: Arc<Mutex<OpcUaClient>>,
    connected: Arc<Mutex<bool>>,
}

impl OpcUaAdapter {
    /// 创建新的 OPC UA 适配器
    pub fn new(config: OpcUaConfig) -> Self {
        Self {
            client: Arc::new(Mutex::new(OpcUaClient::new(config))),
            connected: Arc::new(Mutex::new(false)),
        }
    }
}

#[async_trait]
impl ProtocolClient for OpcUaAdapter {
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
        let client = self.client.lock().await;
        client.read_value(address).await
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
        let client = self.client.lock().await;
        client.write_value(address, value).await
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
        // OPC UA 订阅需要更复杂的实现
        warn!("OPC UA subscription not fully implemented yet");
        Err(anyhow::anyhow!("OPC UA subscription not implemented"))
    }

    async fn unsubscribe(&self, _handle: SubscriptionHandle) -> anyhow::Result<()> {
        Ok(())
    }

    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::OpcUa
    }

    fn is_connected(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcua_adapter_creation() {
        let config = OpcUaConfig::default();
        let adapter = OpcUaAdapter::new(config);
        assert_eq!(adapter.protocol_type(), ProtocolType::OpcUa);
    }
}
