use crate::channel::CommandChannel;
use crate::command::model::DeviceCommand;
use async_trait::async_trait;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// MQTT 指令通道
pub struct MqttCommandChannel {
    /// MQTT 客户端
    client: AsyncClient,
    
    /// 响应接收器
    response_receivers: Arc<RwLock<HashMap<String, mpsc::Sender<serde_json::Value>>>>,
    
    /// 指令主题模板
    command_topic_template: String,
    
    /// 响应主题模板
    response_topic_template: String,
}

impl MqttCommandChannel {
    /// 创建新的 MQTT 指令通道
    pub async fn new(
        broker_host: &str,
        broker_port: u16,
        client_id: &str,
    ) -> anyhow::Result<Self> {
        let mut mqtt_options = MqttOptions::new(client_id, broker_host, broker_port);
        mqtt_options.set_keep_alive(Duration::from_secs(30));
        mqtt_options.set_clean_session(true);

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

        let response_receivers = Arc::new(RwLock::new(HashMap::new()));
        let receivers_clone = response_receivers.clone();

        // 启动事件循环处理响应
        tokio::spawn(async move {
            Self::handle_events(&mut eventloop, receivers_clone).await;
        });

        info!(
            broker = %format!("{}:{}", broker_host, broker_port),
            client_id = %client_id,
            "MQTT command channel created"
        );

        Ok(Self {
            client,
            response_receivers,
            command_topic_template: "device/{device_id}/command".to_string(),
            response_topic_template: "device/{device_id}/response".to_string(),
        })
    }

    /// 处理 MQTT 事件
    async fn handle_events(
        eventloop: &mut EventLoop,
        receivers: Arc<RwLock<HashMap<String, mpsc::Sender<serde_json::Value>>>>,
    ) {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    // 解析响应主题，提取 command_id
                    if let Some(command_id) = Self::extract_command_id(&publish.topic) {
                        // 解析响应数据
                        match serde_json::from_slice::<serde_json::Value>(&publish.payload) {
                            Ok(response) => {
                                debug!(
                                    command_id = %command_id,
                                    "Received command response"
                                );

                                // 发送响应到对应的接收器
                                let receivers_lock = receivers.read().await;
                                if let Some(tx) = receivers_lock.get(&command_id) {
                                    if let Err(e) = tx.send(response).await {
                                        warn!(
                                            command_id = %command_id,
                                            error = %e,
                                            "Failed to send response to receiver"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                error!(
                                    error = %e,
                                    "Failed to parse response payload"
                                );
                            }
                        }
                    }
                }
                Ok(Event::Incoming(packet)) => {
                    debug!(?packet, "Received MQTT packet");
                }
                Ok(Event::Outgoing(_)) => {}
                Err(e) => {
                    error!(error = %e, "MQTT connection error");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// 从响应主题中提取 command_id
    fn extract_command_id(topic: &str) -> Option<String> {
        // 假设响应主题格式: device/{device_id}/response/{command_id}
        topic.split('/').nth(3).map(|s| s.to_string())
    }

    /// 构建指令主题
    fn build_command_topic(&self, device_id: &str) -> String {
        self.command_topic_template
            .replace("{device_id}", device_id)
    }

    /// 构建响应主题
    fn build_response_topic(&self, device_id: &str, command_id: &str) -> String {
        format!(
            "{}/{}",
            self.response_topic_template
                .replace("{device_id}", device_id),
            command_id
        )
    }
}

#[async_trait]
impl CommandChannel for MqttCommandChannel {
    async fn send_command(&self, command: &DeviceCommand) -> anyhow::Result<()> {
        let topic = self.build_command_topic(&command.device_id);
        
        // 序列化指令
        let payload = serde_json::to_vec(command)?;

        // 发布指令
        self.client
            .publish(topic.clone(), QoS::AtLeastOnce, false, payload)
            .await?;

        info!(
            command_id = %command.id,
            device_id = %command.device_id,
            topic = %topic,
            "Command sent via MQTT"
        );

        Ok(())
    }

    async fn wait_response(&self, command_id: &str) -> anyhow::Result<serde_json::Value> {
        // 创建响应接收通道
        let (tx, mut rx) = mpsc::channel(1);
        
        // 注册接收器
        self.response_receivers
            .write()
            .await
            .insert(command_id.to_string(), tx);

        // 等待响应
        match rx.recv().await {
            Some(response) => {
                // 清理接收器
                self.response_receivers
                    .write()
                    .await
                    .remove(command_id);
                Ok(response)
            }
            None => {
                self.response_receivers
                    .write()
                    .await
                    .remove(command_id);
                Err(anyhow::anyhow!("Response channel closed"))
            }
        }
    }

    async fn subscribe_device(&self, device_id: &str) -> anyhow::Result<()> {
        let response_topic = format!("{}/#", self.build_response_topic(device_id, ""));
        
        self.client
            .subscribe(response_topic.clone(), QoS::AtLeastOnce)
            .await?;

        info!(
            device_id = %device_id,
            topic = %response_topic,
            "Subscribed to device responses"
        );

        Ok(())
    }

    async fn unsubscribe_device(&self, device_id: &str) -> anyhow::Result<()> {
        let response_topic = format!("{}/#", self.build_response_topic(device_id, ""));
        
        self.client
            .unsubscribe(response_topic.clone())
            .await?;

        info!(
            device_id = %device_id,
            topic = %response_topic,
            "Unsubscribed from device responses"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_command_id() {
        let topic = "device/device_001/response/cmd_123";
        let command_id = MqttCommandChannel::extract_command_id(topic);
        assert_eq!(command_id, Some("cmd_123".to_string()));
    }

    #[test]
    fn test_build_topics() {
        let channel = MqttCommandChannel {
            client: todo!(),
            response_receivers: Arc::new(RwLock::new(HashMap::new())),
            command_topic_template: "device/{device_id}/command".to_string(),
            response_topic_template: "device/{device_id}/response".to_string(),
        };

        let cmd_topic = channel.build_command_topic("device_001");
        assert_eq!(cmd_topic, "device/device_001/command");

        let resp_topic = channel.build_response_topic("device_001", "cmd_123");
        assert_eq!(resp_topic, "device/device_001/response/cmd_123");
    }
}
