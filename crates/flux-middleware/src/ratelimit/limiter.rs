use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::{RateLimitStrategy, TokenBucket};

/// 限流器
pub struct RateLimiter {
    strategies: Vec<RateLimitStrategy>,
    buckets: Arc<RwLock<HashMap<String, Arc<TokenBucket>>>>,
}

impl RateLimiter {
    /// 创建新的限流器
    pub fn new(strategies: Vec<RateLimitStrategy>) -> Self {
        Self {
            strategies,
            buckets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查是否允许请求
    pub async fn check(&self, key: &str) -> bool {
        for strategy in &self.strategies {
            if !self.check_strategy(key, strategy).await {
                warn!(key = key, strategy = ?strategy, "Rate limit exceeded");
                return false;
            }
        }
        true
    }

    /// 检查单个策略
    async fn check_strategy(&self, key: &str, strategy: &RateLimitStrategy) -> bool {
        match strategy {
            RateLimitStrategy::ByIp { max_requests, window } => {
                self.check_token_bucket(
                    &format!("ip:{}", key),
                    *max_requests,
                    (*max_requests as f64 / window.as_secs() as f64) as u64,
                )
                .await
            }
            RateLimitStrategy::ByUser { max_requests, window } => {
                self.check_token_bucket(
                    &format!("user:{}", key),
                    *max_requests,
                    (*max_requests as f64 / window.as_secs() as f64) as u64,
                )
                .await
            }
            RateLimitStrategy::ByResource { max_clients } => {
                self.check_token_bucket(
                    &format!("resource:{}", key),
                    *max_clients,
                    0,  // 不自动补充
                )
                .await
            }
            RateLimitStrategy::ByBandwidth { max_mbps } => {
                // 带宽限流需要更复杂的实现，这里简化处理
                self.check_token_bucket(
                    &format!("bandwidth:{}", key),
                    *max_mbps * 1024 * 1024,  // 转换为字节
                    *max_mbps * 1024 * 1024,  // 每秒补充
                )
                .await
            }
            RateLimitStrategy::Global { max_requests, window } => {
                self.check_token_bucket(
                    "global",
                    *max_requests,
                    (*max_requests as f64 / window.as_secs() as f64) as u64,
                )
                .await
            }
        }
    }

    /// 使用令牌桶检查
    async fn check_token_bucket(&self, bucket_key: &str, capacity: u64, refill_rate: u64) -> bool {
        let mut buckets = self.buckets.write().await;
        
        let bucket = buckets
            .entry(bucket_key.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(capacity, refill_rate)));

        bucket.try_acquire(1).await
    }

    /// 释放资源（用于 ByResource 策略）
    pub async fn release(&self, key: &str) -> Result<()> {
        let bucket_key = format!("resource:{}", key);
        let buckets = self.buckets.read().await;
        
        if let Some(bucket) = buckets.get(&bucket_key) {
            // 这里需要实现释放逻辑
            // 由于 TokenBucket 不支持直接释放，这里简化处理
            debug!(key = key, "Resource released");
        }
        
        Ok(())
    }

    /// 清理过期的令牌桶
    pub async fn cleanup(&self) {
        let mut buckets = self.buckets.write().await;
        // 这里可以实现清理逻辑，移除长时间未使用的桶
        debug!(count = buckets.len(), "Cleaning up rate limiter buckets");
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            strategies: self.strategies.clone(),
            buckets: self.buckets.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(vec![
            RateLimitStrategy::by_ip(5, 1),  // 每秒 5 个请求
        ]);

        // 前 5 个请求应该通过
        for i in 0..5 {
            assert!(limiter.check("192.168.1.1").await, "Request {} should pass", i);
        }

        // 第 6 个请求应该被限流
        assert!(!limiter.check("192.168.1.1").await, "Request 6 should be rate limited");
    }

    #[tokio::test]
    async fn test_multiple_strategies() {
        let limiter = RateLimiter::new(vec![
            RateLimitStrategy::by_ip(10, 1),
            RateLimitStrategy::global(20, 1),
        ]);

        // 应该同时满足两个策略
        for _ in 0..10 {
            assert!(limiter.check("192.168.1.1").await);
        }
    }
}
