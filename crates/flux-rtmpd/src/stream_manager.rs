use anyhow::Result;
use bytes::Bytes;
use flux_media_core::types::StreamId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info};

/// 流管理器：管理所有活跃的 RTMP 流
pub struct StreamManager {
    streams: Arc<RwLock<HashMap<String, StreamChannel>>>,
}

/// 流通道：用于分发音视频数据到多个订阅者
pub struct StreamChannel {
    pub stream_id: StreamId,
    pub app_name: String,
    pub stream_key: String,
    pub video_tx: broadcast::Sender<MediaPacket>,
    pub audio_tx: broadcast::Sender<MediaPacket>,
    pub subscriber_count: Arc<RwLock<usize>>,
}

/// 媒体数据包
#[derive(Debug, Clone)]
pub struct MediaPacket {
    pub data: Bytes,
    pub timestamp: u32,
    pub is_keyframe: bool,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册新流（发布者）
    pub async fn register_stream(&self, app_name: String, stream_key: String) -> Result<()> {
        let stream_id = StreamId::new("rtmp", &format!("{}/{}", app_name, stream_key));
        let key = format!("{}/{}", app_name, stream_key);

        let (video_tx, _) = broadcast::channel(100);
        let (audio_tx, _) = broadcast::channel(100);

        let channel = StreamChannel {
            stream_id,
            app_name,
            stream_key,
            video_tx,
            audio_tx,
            subscriber_count: Arc::new(RwLock::new(0)),
        };

        let mut streams = self.streams.write().await;
        streams.insert(key.clone(), channel);

        info!(target: "stream_manager", stream_key = %key, "Stream registered");
        Ok(())
    }

    /// 注销流
    pub async fn unregister_stream(&self, app_name: &str, stream_key: &str) -> Result<()> {
        let key = format!("{}/{}", app_name, stream_key);
        let mut streams = self.streams.write().await;
        streams.remove(&key);

        info!(target: "stream_manager", stream_key = %key, "Stream unregistered");
        Ok(())
    }

    /// 发布视频数据
    pub async fn publish_video(
        &self,
        app_name: &str,
        stream_key: &str,
        data: Bytes,
        timestamp: u32,
        is_keyframe: bool,
    ) -> Result<()> {
        let key = format!("{}/{}", app_name, stream_key);
        let streams = self.streams.read().await;

        if let Some(channel) = streams.get(&key) {
            let packet = MediaPacket {
                data,
                timestamp,
                is_keyframe,
            };

            // 忽略发送错误（没有订阅者时会失败）
            let _ = channel.video_tx.send(packet);
            debug!(target: "stream_manager", stream_key = %key, "Video packet published");
        }

        Ok(())
    }

    /// 发布音频数据
    pub async fn publish_audio(
        &self,
        app_name: &str,
        stream_key: &str,
        data: Bytes,
        timestamp: u32,
    ) -> Result<()> {
        let key = format!("{}/{}", app_name, stream_key);
        let streams = self.streams.read().await;

        if let Some(channel) = streams.get(&key) {
            let packet = MediaPacket {
                data,
                timestamp,
                is_keyframe: false,
            };

            let _ = channel.audio_tx.send(packet);
            debug!(target: "stream_manager", stream_key = %key, "Audio packet published");
        }

        Ok(())
    }

    /// 订阅流（播放者）
    pub async fn subscribe(
        &self,
        app_name: &str,
        stream_key: &str,
    ) -> Result<(broadcast::Receiver<MediaPacket>, broadcast::Receiver<MediaPacket>)> {
        let key = format!("{}/{}", app_name, stream_key);
        let streams = self.streams.read().await;

        if let Some(channel) = streams.get(&key) {
            let video_rx = channel.video_tx.subscribe();
            let audio_rx = channel.audio_tx.subscribe();

            // 增加订阅者计数
            let mut count = channel.subscriber_count.write().await;
            *count += 1;

            info!(target: "stream_manager", stream_key = %key, subscribers = *count, "New subscriber");
            Ok((video_rx, audio_rx))
        } else {
            Err(anyhow::anyhow!("Stream not found: {}", key))
        }
    }

    /// 取消订阅
    pub async fn unsubscribe(&self, app_name: &str, stream_key: &str) -> Result<()> {
        let key = format!("{}/{}", app_name, stream_key);
        let streams = self.streams.read().await;

        if let Some(channel) = streams.get(&key) {
            let mut count = channel.subscriber_count.write().await;
            if *count > 0 {
                *count -= 1;
            }

            info!(target: "stream_manager", stream_key = %key, subscribers = *count, "Subscriber left");
        }

        Ok(())
    }

    /// 获取流信息
    pub async fn get_stream_info(&self, app_name: &str, stream_key: &str) -> Option<StreamInfo> {
        let key = format!("{}/{}", app_name, stream_key);
        let streams = self.streams.read().await;

        if let Some(channel) = streams.get(&key) {
            let subscriber_count = *channel.subscriber_count.read().await;
            Some(StreamInfo {
                stream_id: channel.stream_id.clone(),
                app_name: channel.app_name.clone(),
                stream_key: channel.stream_key.clone(),
                subscriber_count,
            })
        } else {
            None
        }
    }

    /// 列出所有流
    pub async fn list_streams(&self) -> Vec<StreamInfo> {
        let streams = self.streams.read().await;
        let mut result = Vec::new();

        for channel in streams.values() {
            let subscriber_count = *channel.subscriber_count.read().await;
            result.push(StreamInfo {
                stream_id: channel.stream_id.clone(),
                app_name: channel.app_name.clone(),
                stream_key: channel.stream_key.clone(),
                subscriber_count,
            });
        }

        result
    }

    /// 检查流是否存在
    pub async fn stream_exists(&self, app_name: &str, stream_key: &str) -> bool {
        let key = format!("{}/{}", app_name, stream_key);
        let streams = self.streams.read().await;
        streams.contains_key(&key)
    }
}

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub stream_id: StreamId,
    pub app_name: String,
    pub stream_key: String,
    pub subscriber_count: usize,
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stream_manager_register() {
        let manager = StreamManager::new();
        manager
            .register_stream("live".to_string(), "test".to_string())
            .await
            .unwrap();

        assert!(manager.stream_exists("live", "test").await);
    }

    #[tokio::test]
    async fn test_stream_manager_subscribe() {
        let manager = StreamManager::new();
        manager
            .register_stream("live".to_string(), "test".to_string())
            .await
            .unwrap();

        let (mut video_rx, _audio_rx) = manager.subscribe("live", "test").await.unwrap();

        // 发布视频数据
        manager
            .publish_video(
                "live",
                "test",
                Bytes::from(vec![1, 2, 3]),
                1000,
                true,
            )
            .await
            .unwrap();

        // 接收数据
        let packet = video_rx.recv().await.unwrap();
        assert_eq!(packet.data, Bytes::from(vec![1, 2, 3]));
        assert_eq!(packet.timestamp, 1000);
        assert!(packet.is_keyframe);
    }

    #[tokio::test]
    async fn test_stream_manager_unregister() {
        let manager = StreamManager::new();
        manager
            .register_stream("live".to_string(), "test".to_string())
            .await
            .unwrap();

        assert!(manager.stream_exists("live", "test").await);

        manager.unregister_stream("live", "test").await.unwrap();

        assert!(!manager.stream_exists("live", "test").await);
    }

    #[tokio::test]
    async fn test_stream_manager_subscriber_count() {
        let manager = StreamManager::new();
        manager
            .register_stream("live".to_string(), "test".to_string())
            .await
            .unwrap();

        let (_rx1, _) = manager.subscribe("live", "test").await.unwrap();
        let (_rx2, _) = manager.subscribe("live", "test").await.unwrap();

        let info = manager.get_stream_info("live", "test").await.unwrap();
        assert_eq!(info.subscriber_count, 2);

        manager.unsubscribe("live", "test").await.unwrap();

        let info = manager.get_stream_info("live", "test").await.unwrap();
        assert_eq!(info.subscriber_count, 1);
    }
}
