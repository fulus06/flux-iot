// 流媒体引擎核心
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use crate::Result;

/// 流媒体引擎：管理所有活跃流
pub struct VideoEngine {
    // 使用 DashMap 实现无锁并发访问
    streams: DashMap<String, Arc<dyn VideoStream>>,
    
    // 全局事件总线
    event_bus: broadcast::Sender<StreamEvent>,
}

impl VideoEngine {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            streams: DashMap::new(),
            event_bus: tx,
        }
    }
    
    /// 发布流（由协议插件调用）
    pub fn publish_stream(&self, stream_id: String, stream: Arc<dyn VideoStream>) -> Result<()> {
        if self.streams.contains_key(&stream_id) {
            return Err(crate::VideoError::StreamAlreadyExists(stream_id));
        }
        
        self.streams.insert(stream_id.clone(), stream);
        
        let _ = self.event_bus.send(StreamEvent::StreamPublished {
            stream_id,
            timestamp: chrono::Utc::now(),
        });
        
        Ok(())
    }
    
    /// 获取所有活跃流
    pub fn list_streams(&self) -> Vec<String> {
        self.streams.iter().map(|entry| entry.key().clone()).collect()
    }
}

impl Default for VideoEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 流抽象 trait
pub trait VideoStream: Send + Sync {
    fn stream_id(&self) -> &str;
}

/// 流事件
#[derive(Clone, Debug)]
pub enum StreamEvent {
    StreamPublished {
        stream_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    StreamClosed {
        stream_id: String,
    },
}
