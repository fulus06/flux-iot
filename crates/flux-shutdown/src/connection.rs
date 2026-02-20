use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// 连接跟踪器
pub struct ConnectionTracker {
    active_connections: Arc<AtomicUsize>,
    is_shutting_down: Arc<AtomicBool>,
    max_drain_duration: Duration,
}

impl ConnectionTracker {
    pub fn new(max_drain_duration: Duration) -> Self {
        Self {
            active_connections: Arc::new(AtomicUsize::new(0)),
            is_shutting_down: Arc::new(AtomicBool::new(false)),
            max_drain_duration,
        }
    }

    /// 尝试获取连接
    pub fn acquire(&self) -> Option<ConnectionGuard> {
        if self.is_shutting_down.load(Ordering::SeqCst) {
            debug!("Rejecting new connection: shutting down");
            return None;
        }

        let count = self.active_connections.fetch_add(1, Ordering::SeqCst);
        debug!("Connection acquired, active: {}", count + 1);
        
        Some(ConnectionGuard::new(self.active_connections.clone()))
    }

    /// 开始关闭
    pub fn start_shutdown(&self) {
        info!("Starting connection drain");
        self.is_shutting_down.store(true, Ordering::SeqCst);
    }

    /// 排空所有连接
    pub async fn drain(&self) {
        self.start_shutdown();
        
        let start = Instant::now();
        let mut last_count = self.active_connections.load(Ordering::SeqCst);

        while self.active_connections.load(Ordering::SeqCst) > 0 {
            let elapsed = start.elapsed();
            
            if elapsed > self.max_drain_duration {
                let remaining = self.active_connections.load(Ordering::SeqCst);
                warn!(
                    "Drain timeout after {:?}, {} connections still active",
                    elapsed, remaining
                );
                break;
            }

            let current_count = self.active_connections.load(Ordering::SeqCst);
            if current_count != last_count {
                info!(
                    "Draining connections: {} remaining ({:?} elapsed)",
                    current_count,
                    elapsed
                );
                last_count = current_count;
            }

            sleep(Duration::from_millis(100)).await;
        }

        let final_count = self.active_connections.load(Ordering::SeqCst);
        if final_count == 0 {
            info!("All connections drained successfully");
        } else {
            warn!("Forced shutdown with {} connections remaining", final_count);
        }
    }

    /// 获取活跃连接数
    pub fn active_count(&self) -> usize {
        self.active_connections.load(Ordering::SeqCst)
    }

    /// 是否正在关闭
    pub fn is_shutting_down(&self) -> bool {
        self.is_shutting_down.load(Ordering::SeqCst)
    }
}

/// 连接守卫
pub struct ConnectionGuard {
    counter: Arc<AtomicUsize>,
}

impl ConnectionGuard {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        Self { counter }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let prev = self.counter.fetch_sub(1, Ordering::SeqCst);
        debug!("Connection released, active: {}", prev - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_tracker() {
        let tracker = ConnectionTracker::new(Duration::from_secs(5));

        // 获取连接
        let _guard1 = tracker.acquire().unwrap();
        let _guard2 = tracker.acquire().unwrap();
        
        assert_eq!(tracker.active_count(), 2);

        // 释放一个连接
        drop(_guard1);
        assert_eq!(tracker.active_count(), 1);

        // 释放所有连接
        drop(_guard2);
        assert_eq!(tracker.active_count(), 0);
    }

    #[tokio::test]
    async fn test_shutdown_rejects_new_connections() {
        let tracker = ConnectionTracker::new(Duration::from_secs(5));

        let _guard = tracker.acquire().unwrap();
        assert_eq!(tracker.active_count(), 1);

        // 开始关闭
        tracker.start_shutdown();

        // 新连接应该被拒绝
        assert!(tracker.acquire().is_none());
    }

    #[tokio::test]
    async fn test_drain_connections() {
        let tracker = ConnectionTracker::new(Duration::from_secs(2));

        let guard1 = tracker.acquire().unwrap();
        let guard2 = tracker.acquire().unwrap();

        // 在后台释放连接
        tokio::spawn(async move {
            sleep(Duration::from_millis(500)).await;
            drop(guard1);
            sleep(Duration::from_millis(500)).await;
            drop(guard2);
        });

        // 排空连接
        tracker.drain().await;

        assert_eq!(tracker.active_count(), 0);
    }

    #[tokio::test]
    async fn test_drain_timeout() {
        let tracker = ConnectionTracker::new(Duration::from_millis(500));

        let _guard = tracker.acquire().unwrap();

        // 排空会超时
        tracker.drain().await;

        // 连接仍然存在（因为 guard 未释放）
        assert_eq!(tracker.active_count(), 1);
    }
}
