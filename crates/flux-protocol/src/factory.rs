use crate::{ProtocolAddress, ProtocolClient, ProtocolType};

/// 协议工厂 trait
/// 
/// 注意：具体实现在 `flux-protocol-factory` 包中，
/// 这里只提供接口定义以避免循环依赖
#[async_trait::async_trait]
pub trait ProtocolFactory: Send + Sync {
    /// 从 URI 创建协议客户端
    async fn from_uri(&self, uri: &str) -> anyhow::Result<Box<dyn ProtocolClient>>;
    
    /// 从地址创建协议客户端
    async fn from_address(&self, address: &ProtocolAddress) -> anyhow::Result<Box<dyn ProtocolClient>>;
}
