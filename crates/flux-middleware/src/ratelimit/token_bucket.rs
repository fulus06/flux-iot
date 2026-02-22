use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

/// 令牌桶限流器
pub struct TokenBucket {
    capacity: u64,
    tokens: Arc<RwLock<TokenState>>,
    refill_rate: u64,  // tokens per second
}

struct TokenState {
    current: u64,
    last_refill: Instant,
}

impl TokenBucket {
    /// 创建新的令牌桶
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            capacity,
            tokens: Arc::new(RwLock::new(TokenState {
                current: capacity,
                last_refill: Instant::now(),
            })),
            refill_rate,
        }
    }

    /// 尝试获取指定数量的令牌（非阻塞）
    pub async fn try_acquire(&self, tokens: u64) -> bool {
        self.refill().await;
        
        let mut state = self.tokens.write().await;
        if state.current >= tokens {
            state.current -= tokens;
            debug!(tokens = tokens, remaining = state.current, "Tokens acquired");
            true
        } else {
            debug!(tokens = tokens, available = state.current, "Insufficient tokens");
            false
        }
    }

    /// 获取指定数量的令牌（阻塞直到可用）
    pub async fn acquire(&self, tokens: u64) -> Result<()> {
        loop {
            if self.try_acquire(tokens).await {
                return Ok(());
            }
            
            // 等待一段时间后重试
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// 补充令牌
    async fn refill(&self) {
        let mut state = self.tokens.write().await;
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill);
        
        let tokens_to_add = (elapsed.as_secs_f64() * self.refill_rate as f64) as u64;
        
        if tokens_to_add > 0 {
            state.current = (state.current + tokens_to_add).min(self.capacity);
            state.last_refill = now;
            debug!(added = tokens_to_add, current = state.current, "Tokens refilled");
        }
    }

    /// 获取当前可用令牌数
    pub async fn available(&self) -> u64 {
        self.refill().await;
        let state = self.tokens.read().await;
        state.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_bucket_acquire() {
        let bucket = TokenBucket::new(10, 5);
        
        // 应该能获取 5 个令牌
        assert!(bucket.try_acquire(5).await);
        
        // 剩余 5 个令牌
        assert_eq!(bucket.available().await, 5);
        
        // 应该能再获取 5 个令牌
        assert!(bucket.try_acquire(5).await);
        
        // 没有剩余令牌
        assert_eq!(bucket.available().await, 0);
        
        // 不应该能获取更多令牌
        assert!(!bucket.try_acquire(1).await);
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let bucket = TokenBucket::new(10, 10);  // 每秒补充 10 个
        
        // 消耗所有令牌
        assert!(bucket.try_acquire(10).await);
        assert_eq!(bucket.available().await, 0);
        
        // 等待 1 秒
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // 应该补充了 10 个令牌
        let available = bucket.available().await;
        assert!(available >= 9 && available <= 10);  // 允许一些时间误差
    }
}
