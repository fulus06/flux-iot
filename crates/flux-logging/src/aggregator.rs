use crate::structured::LogEntry;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error};

/// 日志聚合器
pub struct LogAggregator {
    buffer: Arc<RwLock<Vec<LogEntry>>>,
    max_buffer_size: usize,
    flush_interval: Duration,
    output_path: Option<PathBuf>,
}

impl LogAggregator {
    pub fn new(max_buffer_size: usize, flush_interval_secs: u64) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(Vec::with_capacity(max_buffer_size))),
            max_buffer_size,
            flush_interval: Duration::from_secs(flush_interval_secs),
            output_path: None,
        }
    }

    pub fn with_output_path(mut self, path: PathBuf) -> Self {
        self.output_path = Some(path);
        self
    }

    /// 添加日志条目
    pub async fn add_log(&self, entry: LogEntry) {
        let mut buffer = self.buffer.write().await;
        buffer.push(entry);

        // 如果缓冲区满了，立即刷新
        if buffer.len() >= self.max_buffer_size {
            drop(buffer); // 释放锁
            self.flush().await;
        }
    }

    /// 刷新缓冲区
    pub async fn flush(&self) {
        let mut buffer = self.buffer.write().await;
        if buffer.is_empty() {
            return;
        }

        let logs = std::mem::take(&mut *buffer);
        drop(buffer); // 释放锁

        debug!("Flushing {} log entries", logs.len());

        if let Err(e) = self.write_logs(logs).await {
            error!("Failed to write logs: {}", e);
        }
    }

    async fn write_logs(&self, logs: Vec<LogEntry>) -> Result<(), std::io::Error> {
        if let Some(path) = &self.output_path {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await?;

            for log in logs {
                if let Ok(json) = log.to_json() {
                    file.write_all(json.as_bytes()).await?;
                    file.write_all(b"\n").await?;
                }
            }

            file.flush().await?;
        } else {
            // 输出到 stdout
            for log in logs {
                if let Ok(json) = log.to_json() {
                    println!("{}", json);
                }
            }
        }

        Ok(())
    }

    /// 启动定期刷新
    pub async fn start_periodic_flush(self: Arc<Self>) {
        let mut ticker = interval(self.flush_interval);

        tokio::spawn(async move {
            loop {
                ticker.tick().await;
                self.flush().await;
            }
        });
    }

    /// 获取缓冲区大小
    pub async fn buffer_size(&self) -> usize {
        let buffer = self.buffer.read().await;
        buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structured::{LogEntry, LogLevel};
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_log_aggregator() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let aggregator = LogAggregator::new(10, 60).with_output_path(path.clone());

        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "test".to_string(),
        );

        aggregator.add_log(entry).await;
        assert_eq!(aggregator.buffer_size().await, 1);

        aggregator.flush().await;
        assert_eq!(aggregator.buffer_size().await, 0);

        // 验证文件内容
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("Test message"));
    }

    #[tokio::test]
    async fn test_auto_flush_on_full() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let aggregator = LogAggregator::new(5, 60).with_output_path(path.clone());

        // 添加 5 条日志，应该自动刷新
        for i in 0..5 {
            let entry = LogEntry::new(
                LogLevel::Info,
                format!("Message {}", i),
                "test".to_string(),
            );
            aggregator.add_log(entry).await;
        }

        // 等待刷新完成
        tokio::time::sleep(Duration::from_millis(100)).await;

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("Message 0"));
        assert!(content.contains("Message 4"));
    }
}
