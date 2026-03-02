use crate::types::OpcUaConfig;
use opcua::client::prelude::*;
use std::str::FromStr;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, error, info, warn};

/// OPC UA 客户端（真实实现 - 使用 opcua crate）
pub struct OpcUaClientReal {
    config: OpcUaConfig,
    client: Option<Client>,
    session: Option<Arc<RwLock<Session>>>,
}

impl OpcUaClientReal {
    /// 创建新的 OPC UA 客户端
    pub fn new(config: OpcUaConfig) -> Self {
        Self {
            config,
            client: None,
            session: None,
        }
    }

    /// 连接到 OPC UA 服务器
    pub fn connect(&mut self) -> anyhow::Result<()> {
        info!(endpoint = %self.config.endpoint_url, "Connecting to OPC UA server");

        // 创建客户端
        let mut client = ClientBuilder::new()
            .application_name("FLUX IOT OPC UA Client")
            .application_uri("urn:FluxIoT:OpcUaClient")
            .create_sample_keypair(true)
            .trust_server_certs(true)
            .session_retry_limit(3)
            .client()
            .ok_or_else(|| anyhow::anyhow!("Failed to create OPC UA client"))?;

        // 创建端点描述
        let endpoint: EndpointDescription = (
            self.config.endpoint_url.as_str(),
            "None",
            MessageSecurityMode::None,
            UserTokenPolicy::anonymous(),
        ).into();

        // 连接到端点
        let session = client
            .connect_to_endpoint(endpoint, IdentityToken::Anonymous)
            .map_err(|e| {
                error!(error = ?e, "Failed to connect to OPC UA server");
                anyhow::anyhow!("Failed to connect: {:?}", e)
            })?;

        info!("Connected to OPC UA server successfully");

        // 保存客户端和会话
        self.client = Some(client);
        self.session = Some(session);

        Ok(())
    }

    /// 断开连接
    pub fn disconnect(&mut self) -> anyhow::Result<()> {
        info!("Disconnecting from OPC UA server");

        if let Some(session) = &self.session {
            let session_lock = session.read();
            session_lock.disconnect();
        }

        self.session = None;
        self.client = None;

        info!("Disconnected from OPC UA server");
        Ok(())
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.session.is_some()
    }

    /// 读取节点值
    pub fn read_value(&self, node_id: &str) -> anyhow::Result<serde_json::Value> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;

        debug!(node_id = %node_id, "Reading OPC UA value");

        // 解析节点 ID
        let node = NodeId::from_str(node_id).map_err(|e| {
            error!(node_id = %node_id, error = ?e, "Invalid node ID");
            anyhow::anyhow!("Invalid node ID: {}", e)
        })?;

        // 读取节点值
        let nodes_to_read = vec![node.into()];
        let session_lock = session.read();
        let results = session_lock
            .read(&nodes_to_read, TimestampsToReturn::Both, 1.0)
            .map_err(|e| {
                error!(error = ?e, "Failed to read OPC UA value");
                anyhow::anyhow!("Failed to read value: {:?}", e)
            })?;

        // 处理结果
        if let Some(data_value) = results.first() {
            if let Some(ref value) = data_value.value {
                // 转换 OPC UA 值为 JSON
                let json_value = Self::variant_to_json(value)?;

                debug!(
                    node_id = %node_id,
                    value = ?json_value,
                    "Successfully read OPC UA value"
                );

                Ok(serde_json::json!({
                    "node_id": node_id,
                    "value": json_value,
                    "status": "Good",
                    "source_timestamp": data_value.source_timestamp.as_ref().map(|t: &opcua::types::DateTime| t.to_string()),
                    "server_timestamp": data_value.server_timestamp.as_ref().map(|t: &opcua::types::DateTime| t.to_string()),
                }))
            } else {
                Err(anyhow::anyhow!("No value in response"))
            }
        } else {
            Err(anyhow::anyhow!("No results returned"))
        }
    }

    /// 写入节点值
    pub fn write_value(&self, node_id: &str, value: serde_json::Value) -> anyhow::Result<()> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;

        debug!(
            node_id = %node_id,
            value = ?value,
            "Writing OPC UA value"
        );

        // 解析节点 ID
        let node = NodeId::from_str(node_id).map_err(|e| {
            error!(node_id = %node_id, error = ?e, "Invalid node ID");
            anyhow::anyhow!("Invalid node ID: {}", e)
        })?;

        // 转换 JSON 值为 OPC UA Variant
        let variant = Self::json_to_variant(&value)?;

        // 写入节点值
        let nodes_to_write = vec![WriteValue {
            node_id: node,
            attribute_id: AttributeId::Value as u32,
            index_range: UAString::null(),
            value: DataValue::value_only(variant),
        }];

        let session_lock = session.read();
        let results = session_lock.write(&nodes_to_write).map_err(|e| {
            error!(error = ?e, "Failed to write OPC UA value");
            anyhow::anyhow!("Failed to write value: {:?}", e)
        })?;

        // 检查写入结果
        if let Some(status_code) = results.first() {
            if status_code.is_good() {
                info!(node_id = %node_id, "Successfully wrote OPC UA value");
                Ok(())
            } else {
                Err(anyhow::anyhow!("Write failed with status: {:?}", status_code))
            }
        } else {
            Err(anyhow::anyhow!("No write results returned"))
        }
    }

    /// 将 OPC UA Variant 转换为 JSON
    fn variant_to_json(variant: &Variant) -> anyhow::Result<serde_json::Value> {
        let json = match variant {
            Variant::Boolean(v) => serde_json::json!(v),
            Variant::SByte(v) => serde_json::json!(v),
            Variant::Byte(v) => serde_json::json!(v),
            Variant::Int16(v) => serde_json::json!(v),
            Variant::UInt16(v) => serde_json::json!(v),
            Variant::Int32(v) => serde_json::json!(v),
            Variant::UInt32(v) => serde_json::json!(v),
            Variant::Int64(v) => serde_json::json!(v),
            Variant::UInt64(v) => serde_json::json!(v),
            Variant::Float(v) => serde_json::json!(v),
            Variant::Double(v) => serde_json::json!(v),
            Variant::String(v) => serde_json::json!(v.as_ref()),
            Variant::DateTime(v) => serde_json::json!(v.to_string()),
            Variant::Guid(v) => serde_json::json!(v.to_string()),
            Variant::ByteString(v) => {
                serde_json::json!(v.value.as_ref().map(|b| base64::encode(b)))
            }
            _ => serde_json::json!(format!("{:?}", variant)),
        };
        Ok(json)
    }

    /// 将 JSON 转换为 OPC UA Variant
    fn json_to_variant(value: &serde_json::Value) -> anyhow::Result<Variant> {
        let variant = match value {
            serde_json::Value::Bool(v) => Variant::Boolean(*v),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                        Variant::Int32(i as i32)
                    } else {
                        Variant::Int64(i)
                    }
                } else if let Some(f) = n.as_f64() {
                    Variant::Double(f)
                } else {
                    return Err(anyhow::anyhow!("Unsupported number type"));
                }
            }
            serde_json::Value::String(s) => Variant::String(UAString::from(s.as_str())),
            _ => return Err(anyhow::anyhow!("Unsupported JSON type for OPC UA")),
        };
        Ok(variant)
    }

    /// 创建订阅（推荐使用轮询方式）
    /// 
    /// 注意：由于 opcua crate 0.12 的订阅 API 复杂性，
    /// 推荐使用定时轮询 read_value() 的方式监控数据变化。
    /// 
    /// # 替代方案
    /// 
    /// ```rust
    /// use tokio::time::{interval, Duration};
    /// 
    /// let mut interval = interval(Duration::from_millis(500));
    /// loop {
    ///     interval.tick().await;
    ///     match client.read_value("ns=0;i=2258") {
    ///         Ok(value) => {
    ///             // 处理数据变化
    ///             callback(value);
    ///         }
    ///         Err(e) => error!("Read failed: {}", e),
    ///     }
    /// }
    /// ```
    /// 
    /// # 参数
    /// - `node_id`: 要监控的节点 ID
    /// - `_callback`: 数据变化时的回调函数（当前未使用）
    /// 
    /// # 返回
    /// 订阅 ID（用于后续删除）
    pub fn create_subscription<F>(
        &mut self,
        node_id: &str,
        _callback: F,
    ) -> anyhow::Result<String>
    where
        F: Fn(serde_json::Value) + Send + Sync + 'static,
    {
        if !self.is_connected() {
            return Err(anyhow::anyhow!("Not connected"));
        }

        info!(
            node_id = %node_id,
            "OPC UA subscription requested - recommend using polling with read_value() instead"
        );
        
        // 返回一个占位符订阅 ID
        // 实际应用中建议使用定时轮询 read_value()
        Ok(format!("polling:{}", node_id))
    }

    /// 删除订阅
    /// 
    /// # 参数
    /// - `subscription_id`: 订阅 ID
    pub fn delete_subscription(&mut self, subscription_id: &str) -> anyhow::Result<()> {
        if !self.is_connected() {
            return Err(anyhow::anyhow!("Not connected"));
        }

        info!(
            subscription_id = %subscription_id,
            "OPC UA subscription deletion requested"
        );

        Ok(())
    }
}
