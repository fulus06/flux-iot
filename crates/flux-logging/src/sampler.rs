use crate::structured::LogLevel;
use rand::Rng;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 采样策略
#[derive(Debug, Clone)]
pub enum SamplingStrategy {
    /// 始终记录
    Always,
    
    /// 从不记录
    Never,
    
    /// 按比例采样（0.0-1.0）
    Ratio(f64),
    
    /// 按级别采样
    ByLevel {
        trace: f64,
        debug: f64,
        info: f64,
        warn: f64,
        error: f64,
    },
    
    /// 速率限制（每秒最多 N 条）
    RateLimit(u32),
    
    /// 自适应采样
    Adaptive {
        base_rate: f64,
        max_rate: f64,
        error_boost: f64,
    },
}

/// 日志采样器
pub struct LogSampler {
    strategy: SamplingStrategy,
    counter: AtomicU64,
    last_reset: Arc<RwLock<Instant>>,
    error_count: AtomicU64,
}

impl LogSampler {
    pub fn new(strategy: SamplingStrategy) -> Self {
        Self {
            strategy,
            counter: AtomicU64::new(0),
            last_reset: Arc::new(RwLock::new(Instant::now())),
            error_count: AtomicU64::new(0),
        }
    }

    /// 判断是否应该采样
    pub async fn should_sample(&self, level: LogLevel) -> bool {
        match &self.strategy {
            SamplingStrategy::Always => true,
            
            SamplingStrategy::Never => false,
            
            SamplingStrategy::Ratio(ratio) => {
                rand::thread_rng().gen::<f64>() < *ratio
            }
            
            SamplingStrategy::ByLevel {
                trace,
                debug,
                info,
                warn,
                error,
            } => {
                let ratio = match level {
                    LogLevel::Trace => *trace,
                    LogLevel::Debug => *debug,
                    LogLevel::Info => *info,
                    LogLevel::Warn => *warn,
                    LogLevel::Error => *error,
                };
                rand::thread_rng().gen::<f64>() < ratio
            }
            
            SamplingStrategy::RateLimit(max_per_sec) => {
                self.check_rate_limit(*max_per_sec).await
            }
            
            SamplingStrategy::Adaptive {
                base_rate,
                max_rate,
                error_boost,
            } => {
                self.adaptive_sample(level, *base_rate, *max_rate, *error_boost)
                    .await
            }
        }
    }

    async fn check_rate_limit(&self, max_per_sec: u32) -> bool {
        let now = Instant::now();
        let mut last_reset = self.last_reset.write().await;

        // 每秒重置计数器
        if now.duration_since(*last_reset) >= Duration::from_secs(1) {
            self.counter.store(0, Ordering::Relaxed);
            *last_reset = now;
        }

        let count = self.counter.fetch_add(1, Ordering::Relaxed);
        count < max_per_sec as u64
    }

    async fn adaptive_sample(
        &self,
        level: LogLevel,
        base_rate: f64,
        max_rate: f64,
        error_boost: f64,
    ) -> bool {
        // 错误日志总是记录
        if level == LogLevel::Error {
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return true;
        }

        // 根据错误数量动态调整采样率
        let error_count = self.error_count.load(Ordering::Relaxed);
        let adjusted_rate = if error_count > 0 {
            (base_rate + error_boost * (error_count as f64 / 100.0)).min(max_rate)
        } else {
            base_rate
        };

        rand::thread_rng().gen::<f64>() < adjusted_rate
    }

    /// 重置错误计数
    pub fn reset_error_count(&self) {
        self.error_count.store(0, Ordering::Relaxed);
    }

    /// 获取当前计数
    pub fn get_count(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
    }
}

impl Default for LogSampler {
    fn default() -> Self {
        Self::new(SamplingStrategy::Always)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_always_sampler() {
        let sampler = LogSampler::new(SamplingStrategy::Always);
        assert!(sampler.should_sample(LogLevel::Info).await);
        assert!(sampler.should_sample(LogLevel::Debug).await);
    }

    #[tokio::test]
    async fn test_never_sampler() {
        let sampler = LogSampler::new(SamplingStrategy::Never);
        assert!(!sampler.should_sample(LogLevel::Info).await);
        assert!(!sampler.should_sample(LogLevel::Debug).await);
    }

    #[tokio::test]
    async fn test_ratio_sampler() {
        let sampler = LogSampler::new(SamplingStrategy::Ratio(0.5));
        
        let mut sampled = 0;
        for _ in 0..1000 {
            if sampler.should_sample(LogLevel::Info).await {
                sampled += 1;
            }
        }
        
        // 应该接近 500（允许一些误差）
        assert!(sampled > 400 && sampled < 600);
    }

    #[tokio::test]
    async fn test_rate_limit_sampler() {
        let sampler = LogSampler::new(SamplingStrategy::RateLimit(10));
        
        let mut sampled = 0;
        for _ in 0..20 {
            if sampler.should_sample(LogLevel::Info).await {
                sampled += 1;
            }
        }
        
        // 应该最多 10 条
        assert!(sampled <= 10);
    }

    #[tokio::test]
    async fn test_by_level_sampler() {
        let sampler = LogSampler::new(SamplingStrategy::ByLevel {
            trace: 0.0,
            debug: 0.0,
            info: 1.0,
            warn: 1.0,
            error: 1.0,
        });
        
        assert!(sampler.should_sample(LogLevel::Info).await);
        assert!(sampler.should_sample(LogLevel::Error).await);
    }

    #[tokio::test]
    async fn test_adaptive_sampler() {
        let sampler = LogSampler::new(SamplingStrategy::Adaptive {
            base_rate: 0.1,
            max_rate: 0.5,
            error_boost: 0.1,
        });
        
        // 错误日志总是记录
        assert!(sampler.should_sample(LogLevel::Error).await);
    }
}
