use crate::client::CoapClient;
use crate::types::CoapConfig;
use async_trait::async_trait;
use flux_protocol::{ProtocolClient, ProtocolType, SubscriptionHandle};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

/// CoAP 协议适配器
pub struct CoapAdapter {
    client: Arc<Mutex<CoapClient>>,
    connected: Arc<Mutex<bool>>,
}

impl CoapAdapter {
    /// 创建新的 CoAP 适配器
    pub fn new(config: CoapConfig) -> Self {
        Self {
            client: Arc::new(Mutex::new(CoapClient::new(config))),
            connected: Arc::new(Mutex::new(false)),
        }
    }
}

#[async_trait]
impl ProtocolClient for CoapAdapter {
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
        
        // CoAP GET 请求
        let payload = client.get(address).await?;
        
        // 尝试解析为 JSON
        if let Ok(value) = serde_json::from_slice::<Value>(&payload) {
            Ok(value)
        } else {
            // 如果不是 JSON，返回字符串
            let text = String::from_utf8_lossy(&payload).to_string();
            Ok(serde_json::json!(text))
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
        let client = self.client.lock().await;
        
        // 将 JSON 转换为字节
        let payload = serde_json::to_vec(&value)?;
        
        // CoAP PUT 请求
        client.put(address, payload).await?;
        
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
        // CoAP Observe 功能需要更复杂的实现
        // 这里先返回未实现错误
        warn!("CoAP Observe not fully implemented yet");
        Err(anyhow::anyhow!("CoAP Observe not implemented"))
    }

    async fn unsubscribe(&self, _handle: SubscriptionHandle) -> anyhow::Result<()> {
        Ok(())
    }

    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::CoAP
    }

    fn is_connected(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coap_adapter_creation() {
        let config = CoapConfig::default();
        let adapter = CoapAdapter::new(config);
        assert_eq!(adapter.protocol_type(), ProtocolType::CoAP);
    }
}
