use crate::client::OpcUaClient;
use crate::types::OpcUaConfig;
use async_trait::async_trait;
use flux_protocol::{ProtocolClient, ProtocolType, SubscriptionHandle};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// OPC UA 协议适配器
pub struct OpcUaAdapter {
    client: Arc<RwLock<OpcUaClient>>,
    connected: Arc<RwLock<bool>>,
    /// 订阅句柄映射 (handle_id -> subscription_id)
    subscriptions: Arc<RwLock<HashMap<String, String>>>,
}

impl OpcUaAdapter {
    /// 创建新的 OPC UA 适配器
    pub fn new(config: OpcUaConfig) -> Self {
        Self {
            client: Arc::new(RwLock::new(OpcUaClient::new(config))),
            connected: Arc::new(RwLock::new(false)),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ProtocolClient for OpcUaAdapter {
    async fn connect(&mut self) -> anyhow::Result<()> {
        let mut client = self.client.write().await;
        client.connect().await?;
        *self.connected.write().await = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        let mut client = self.client.write().await;
        client.disconnect().await?;
        *self.connected.write().await = false;
        self.subscriptions.write().await.clear();
        Ok(())
    }

    async fn read(&self, address: &str) -> anyhow::Result<Value> {
        let client = self.client.read().await;
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
        let client = self.client.read().await;
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
        address: &str,
        callback: Box<dyn Fn(Value) + Send + Sync>,
    ) -> anyhow::Result<SubscriptionHandle> {
        let mut client = self.client.write().await;
        
        // 创建 OPC UA 订阅
        let subscription_id = client.create_subscription(address, move |value| {
            callback(value);
        }).await?;
        
        // 生成订阅句柄 ID
        let handle_id = uuid::Uuid::new_v4().to_string();
        
        // 保存映射
        self.subscriptions.write().await.insert(handle_id.clone(), subscription_id);
        
        debug!(handle_id = %handle_id, address = %address, "OPC UA subscription created");
        
        Ok(SubscriptionHandle::new(handle_id))
    }

    async fn unsubscribe(&self, handle: SubscriptionHandle) -> anyhow::Result<()> {
        let mut subs = self.subscriptions.write().await;
        
        if let Some(subscription_id) = subs.remove(&handle.id) {
            let mut client = self.client.write().await;
            client.delete_subscription(&subscription_id).await?;
            debug!(handle_id = %handle.id, "OPC UA subscription deleted");
        }
        
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
