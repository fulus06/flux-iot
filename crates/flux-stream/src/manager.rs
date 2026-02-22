use crate::context::StreamContext;
use crate::stream::{ClientInfo, OutputStream, Protocol, QualityLevel, Stream};
use crate::trigger::TriggerDetector;
use anyhow::{anyhow, Result};
use flux_config::{StreamMode, StreamingConfig};
use flux_media_core::types::StreamId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// 统一流管理器（协议无关）
pub struct StreamManager {
    contexts: Arc<RwLock<HashMap<StreamId, StreamContext>>>,
    streams: Arc<RwLock<HashMap<StreamId, Box<dyn Stream>>>>,
    config: StreamingConfig,
    trigger_detector: TriggerDetector,
}

impl StreamManager {
    pub fn new(config: StreamingConfig) -> Self {
        Self {
            contexts: Arc::new(RwLock::new(HashMap::new())),
            streams: Arc::new(RwLock::new(HashMap::new())),
            config,
            trigger_detector: TriggerDetector::new(),
        }
    }

    /// 注册流（任何协议）
    pub async fn register_stream(
        &self,
        stream: Box<dyn Stream>,
        mode: StreamMode,
    ) -> Result<()> {
        let stream_id = stream.stream_id().clone();
        let protocol = stream.protocol();

        let context = StreamContext::new(stream_id.clone(), protocol, mode);

        let mut contexts = self.contexts.write().await;
        let mut streams = self.streams.write().await;

        contexts.insert(stream_id.clone(), context);
        streams.insert(stream_id.clone(), stream);

        info!(
            stream_id = %stream_id,
            protocol = %protocol,
            "Stream registered"
        );

        Ok(())
    }

    /// 注销流
    pub async fn unregister_stream(&self, stream_id: &StreamId) -> Result<()> {
        let mut contexts = self.contexts.write().await;
        let mut streams = self.streams.write().await;

        contexts.remove(stream_id);
        streams.remove(stream_id);

        info!(stream_id = %stream_id, "Stream unregistered");
        Ok(())
    }

    /// 请求输出流（自动检测是否需要转码）
    pub async fn request_output(
        &self,
        stream_id: &StreamId,
        client_info: ClientInfo,
    ) -> Result<OutputStream> {
        let contexts = self.contexts.read().await;
        let context = contexts
            .get(stream_id)
            .ok_or_else(|| anyhow!("Stream not found: {}", stream_id))?
            .clone();
        drop(contexts);

        context.add_client(client_info.clone()).await;

        match &context.mode {
            StreamMode::Auto { triggers } => {
                let should_transcode = self
                    .trigger_detector
                    .evaluate(&context, &client_info, triggers)
                    .await?;

                if should_transcode && !context.is_transcoding {
                    info!(
                        stream_id = %stream_id,
                        "Auto-triggering transcode"
                    );
                    self.switch_to_transcode(stream_id).await?;
                }
            }
            _ => {}
        }

        self.get_output_stream(stream_id, &client_info).await
    }

    /// 获取输出流
    async fn get_output_stream(
        &self,
        stream_id: &StreamId,
        client_info: &ClientInfo,
    ) -> Result<OutputStream> {
        let protocol = client_info.preferred_protocol;
        let url = self.generate_output_url(stream_id, protocol).await?;

        Ok(OutputStream {
            stream_id: stream_id.clone(),
            protocol,
            url,
            quality: QualityLevel::Auto,
        })
    }

    /// 生成输出 URL
    async fn generate_output_url(&self, stream_id: &StreamId, protocol: Protocol) -> Result<String> {
        let identifier = stream_id.identifier().unwrap_or(stream_id.as_str());
        let base_url = match protocol {
            Protocol::RTMP => format!("rtmp://localhost:1935/live/{}", identifier),
            Protocol::RTSP => format!("rtsp://localhost:8554/{}", identifier),
            Protocol::HttpFlv => format!("http://localhost:8080/flv/{}.flv", identifier),
            Protocol::WebRTC => format!("webrtc://localhost:8443/{}", identifier),
            Protocol::SRT => format!("srt://localhost:9000?streamid={}", identifier),
        };
        Ok(base_url)
    }

    /// 切换到转码模式
    async fn switch_to_transcode(&self, stream_id: &StreamId) -> Result<()> {
        let mut contexts = self.contexts.write().await;
        
        if let Some(context) = contexts.get_mut(stream_id) {
            if context.is_transcoding {
                return Ok(());
            }

            info!(stream_id = %stream_id, "Switching to transcode mode");
            
            context.is_transcoding = true;

            Ok(())
        } else {
            Err(anyhow!("Stream not found: {}", stream_id))
        }
    }

    /// 获取流上下文
    pub async fn get_context(&self, stream_id: &StreamId) -> Option<StreamContext> {
        let contexts = self.contexts.read().await;
        contexts.get(stream_id).cloned()
    }

    /// 列出所有流
    pub async fn list_streams(&self) -> Vec<StreamId> {
        let contexts = self.contexts.read().await;
        contexts.keys().cloned().collect()
    }

    /// 获取流数量
    pub async fn stream_count(&self) -> usize {
        let contexts = self.contexts.read().await;
        contexts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ClientType, StreamMetadata, StreamStatus};
    use async_trait::async_trait;
    use flux_config::TranscodeTrigger;

    struct MockStream {
        stream_id: StreamId,
        protocol: Protocol,
        metadata: StreamMetadata,
        status: StreamStatus,
    }

    #[async_trait]
    impl Stream for MockStream {
        fn stream_id(&self) -> &StreamId {
            &self.stream_id
        }

        fn protocol(&self) -> Protocol {
            self.protocol
        }

        async fn metadata(&self) -> StreamMetadata {
            self.metadata.clone()
        }

        async fn status(&self) -> StreamStatus {
            self.status
        }

        async fn start(&mut self) -> Result<()> {
            self.status = StreamStatus::Running;
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            self.status = StreamStatus::Stopped;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_register_and_unregister_stream() {
        let config = StreamingConfig::default();
        let manager = StreamManager::new(config);

        let stream = Box::new(MockStream {
            stream_id: StreamId::new("test", "stream-001"),
            protocol: Protocol::RTMP,
            metadata: StreamMetadata::default(),
            status: StreamStatus::Idle,
        });

        let stream_id = stream.stream_id().clone();

        manager
            .register_stream(stream, StreamMode::Passthrough { remux: true })
            .await
            .unwrap();

        assert_eq!(manager.stream_count().await, 1);

        manager.unregister_stream(&stream_id).await.unwrap();

        assert_eq!(manager.stream_count().await, 0);
    }

    #[tokio::test]
    async fn test_auto_transcode_trigger() {
        let config = StreamingConfig::default();
        let manager = StreamManager::new(config);

        let stream = Box::new(MockStream {
            stream_id: StreamId::new("test", "stream-001"),
            protocol: Protocol::RTSP,
            metadata: StreamMetadata::default(),
            status: StreamStatus::Idle,
        });

        let stream_id = stream.stream_id().clone();

        manager
            .register_stream(
                stream,
                StreamMode::Auto {
                    triggers: vec![TranscodeTrigger::ProtocolSwitch],
                },
            )
            .await
            .unwrap();

        // 第一个客户端请求 RTMP
        let client1 = ClientInfo {
            client_id: "client-1".to_string(),
            client_type: ClientType::WebBrowser,
            preferred_protocol: Protocol::RTMP,
            bandwidth_estimate: Some(2000),
            user_agent: None,
        };

        let _output1 = manager.request_output(&stream_id, client1).await.unwrap();

        // 此时不应该转码（只有一种协议）
        let context = manager.get_context(&stream_id).await.unwrap();
        assert!(!context.is_transcoding);

        // 第二个客户端请求不同协议 HTTP-FLV
        let client2 = ClientInfo {
            client_id: "client-2".to_string(),
            client_type: ClientType::MobileApp,
            preferred_protocol: Protocol::HttpFlv,
            bandwidth_estimate: Some(1000),
            user_agent: None,
        };

        let _output2 = manager.request_output(&stream_id, client2).await.unwrap();

        // 现在应该触发转码（检测到协议切换）
        let context = manager.get_context(&stream_id).await.unwrap();
        assert!(context.is_transcoding);
    }
}
