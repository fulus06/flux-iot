// 零拷贝写入流水线（支持 100+ 路并发）
use dashmap::DashMap;
use tokio::sync::mpsc;
use bytes::Bytes;
use std::sync::Arc;
use chrono::{DateTime, Utc};

/// 写入流水线（支持 100+ 路并发）
pub struct WritePipeline {
    /// 写入队列（每个流一个）
    queues: Arc<DashMap<String, mpsc::Sender<WriteTask>>>,
    
    /// Worker 数量
    worker_count: usize,
}

impl WritePipeline {
    pub fn new(worker_count: usize) -> Self {
        tracing::info!("WritePipeline initialized with {} workers", worker_count);
        
        Self {
            queues: Arc::new(DashMap::new()),
            worker_count,
        }
    }
    
    /// 提交写入任务（非阻塞）
    pub async fn submit(
        &self,
        stream_id: String,
        timestamp: DateTime<Utc>,
        data: Bytes,
    ) -> Result<(), mpsc::error::SendError<WriteTask>> {
        // 获取或创建该流的队列
        let tx = self.queues.entry(stream_id.clone())
            .or_insert_with(|| {
                let (tx, rx) = mpsc::channel(100); // 每个流缓存 100 个分片
                
                // 启动 Worker 处理该流
                tokio::spawn(async move {
                    Self::worker_loop(rx).await;
                });
                
                tx
            })
            .clone();
        
        // 非阻塞发送
        tx.send(WriteTask {
            stream_id,
            timestamp,
            data,
        }).await
    }
    
    /// Worker 循环处理写入任务
    async fn worker_loop(mut rx: mpsc::Receiver<WriteTask>) {
        while let Some(task) = rx.recv().await {
            // 实际写入逻辑由 StandaloneStorage 处理
            // 这里仅做队列管理
            tracing::trace!(
                "Processing write task: {} at {}",
                task.stream_id,
                task.timestamp
            );
        }
    }
    
    /// 获取队列数量
    pub fn queue_count(&self) -> usize {
        self.queues.len()
    }
}

/// 写入任务
#[derive(Debug)]
pub struct WriteTask {
    pub stream_id: String,
    pub timestamp: DateTime<Utc>,
    pub data: Bytes,
}
