use crate::types::{ProtocolType, SubscriptionHandle};
use async_trait::async_trait;
use serde_json::Value;

/// 统一协议客户端接口
#[async_trait]
pub trait ProtocolClient: Send + Sync {
    /// 连接设备
    async fn connect(&mut self) -> anyhow::Result<()>;
    
    /// 断开连接
    async fn disconnect(&mut self) -> anyhow::Result<()>;
    
    /// 读取单个数据点
    async fn read(&self, address: &str) -> anyhow::Result<Value>;
    
    /// 批量读取数据点
    async fn read_multiple(&self, addresses: &[String]) -> anyhow::Result<Vec<Value>>;
    
    /// 写入单个数据点
    async fn write(&self, address: &str, value: Value) -> anyhow::Result<()>;
    
    /// 批量写入数据点
    async fn write_multiple(&self, data: &[(String, Value)]) -> anyhow::Result<()>;
    
    /// 订阅数据变化
    async fn subscribe(
        &self,
        address: &str,
        callback: Box<dyn Fn(Value) + Send + Sync>,
    ) -> anyhow::Result<SubscriptionHandle>;
    
    /// 取消订阅
    async fn unsubscribe(&self, handle: SubscriptionHandle) -> anyhow::Result<()>;
    
    /// 获取协议类型
    fn protocol_type(&self) -> ProtocolType;
    
    /// 检查连接状态
    fn is_connected(&self) -> bool;
    
    /// 获取设备信息
    async fn get_device_info(&self) -> anyhow::Result<Value> {
        Ok(serde_json::json!({
            "protocol": self.protocol_type().as_str(),
            "connected": self.is_connected(),
        }))
    }
}
