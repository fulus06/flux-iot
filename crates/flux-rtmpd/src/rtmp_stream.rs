use async_trait::async_trait;
use flux_media_core::types::StreamId;
use flux_stream::{Protocol, Stream, StreamMetadata, StreamStatus};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::stream_manager::StreamChannel;

/// RTMP 流实现（实现 flux-stream 的 Stream trait）
pub struct RtmpStream {
    stream_id: StreamId,
    app_name: String,
    stream_key: String,
    metadata: Arc<RwLock<StreamMetadata>>,
    status: Arc<RwLock<StreamStatus>>,
    channel: Arc<StreamChannel>,
}

impl RtmpStream {
    pub fn new(
        app_name: String,
        stream_key: String,
        channel: Arc<StreamChannel>,
    ) -> Self {
        let stream_id = StreamId::new("rtmp", &format!("{}/{}", app_name, stream_key));
        
        Self {
            stream_id,
            app_name,
            stream_key,
            metadata: Arc::new(RwLock::new(StreamMetadata::default())),
            status: Arc::new(RwLock::new(StreamStatus::Idle)),
            channel,
        }
    }

    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    pub fn stream_key(&self) -> &str {
        &self.stream_key
    }

    pub fn channel(&self) -> &Arc<StreamChannel> {
        &self.channel
    }

    pub async fn update_metadata(&self, metadata: StreamMetadata) {
        let mut meta = self.metadata.write().await;
        *meta = metadata;
    }
}

#[async_trait]
impl Stream for RtmpStream {
    fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    fn protocol(&self) -> Protocol {
        Protocol::RTMP
    }

    async fn metadata(&self) -> StreamMetadata {
        let meta = self.metadata.read().await;
        meta.clone()
    }

    async fn status(&self) -> StreamStatus {
        let status = self.status.read().await;
        *status
    }

    async fn start(&mut self) -> anyhow::Result<()> {
        let mut status = self.status.write().await;
        *status = StreamStatus::Running;
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        let mut status = self.status.write().await;
        *status = StreamStatus::Stopped;
        Ok(())
    }
}
