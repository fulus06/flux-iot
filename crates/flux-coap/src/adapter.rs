use crate::client::CoapClient;
use crate::types::CoapConfig;
use async_trait::async_trait;
use flux_protocol::{ProtocolClient, ProtocolType, SubscriptionHandle};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// CoAP 协议适配器
pub struct CoapAdapter {
    client: Arc<RwLock<CoapClient>>,
    connected: Arc<RwLock<bool>>,
    /// 订阅句柄映射 (handle_id -> token)
    subscriptions: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl CoapAdapter {
    /// 创建新的 CoAP 适配器
    pub fn new(config: CoapConfig) -> Self {
        Self {
            client: Arc::new(RwLock::new(CoapClient::new(config))),
            connected: Arc::new(RwLock::new(false)),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ProtocolClient for CoapAdapter {
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
        let client = self.client.read().await;
        
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
        address: &str,
        callback: Box<dyn Fn(Value) + Send + Sync>,
    ) -> anyhow::Result<SubscriptionHandle> {
        let mut client = self.client.write().await;
        
        // 使用 CoAP Observe 订阅资源
        let token = client.observe(address, move |payload| {
            // 解析 payload 为 JSON
            if let Ok(value) = serde_json::from_slice::<Value>(&payload) {
                callback(value);
            } else {
                // 如果不是 JSON，返回字符串
                let text = String::from_utf8_lossy(&payload).to_string();
                callback(serde_json::json!(text));
            }
        }).await?;
        
        // 生成订阅句柄 ID
        let handle_id = uuid::Uuid::new_v4().to_string();
        
        // 保存 token 映射
        self.subscriptions.write().await.insert(handle_id.clone(), token);
        
        debug!(handle_id = %handle_id, address = %address, "CoAP Observe subscription created");
        
        Ok(SubscriptionHandle::new(handle_id))
    }

    async fn unsubscribe(&self, handle: SubscriptionHandle) -> anyhow::Result<()> {
        let mut subs = self.subscriptions.write().await;
        
        if let Some(token) = subs.remove(&handle.id) {
            let mut client = self.client.write().await;
            client.cancel_observe(&token).await?;
            debug!(handle_id = %handle.id, "CoAP Observe subscription cancelled");
        }
        
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
