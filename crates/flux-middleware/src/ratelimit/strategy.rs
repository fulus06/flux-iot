use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 限流策略
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RateLimitStrategy {
    /// 按 IP 地址限流
    ByIp {
        max_requests: u64,
        window: Duration,
    },
    
    /// 按用户 ID 限流
    ByUser {
        max_requests: u64,
        window: Duration,
    },
    
    /// 按资源限流（如流 ID）
    ByResource {
        max_clients: u64,
    },
    
    /// 带宽限流
    ByBandwidth {
        max_mbps: u64,
    },
    
    /// 全局限流
    Global {
        max_requests: u64,
        window: Duration,
    },
}

impl RateLimitStrategy {
    /// 创建按 IP 限流策略
    pub fn by_ip(max_requests: u64, window_secs: u64) -> Self {
        Self::ByIp {
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// 创建按用户限流策略
    pub fn by_user(max_requests: u64, window_secs: u64) -> Self {
        Self::ByUser {
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// 创建按资源限流策略
    pub fn by_resource(max_clients: u64) -> Self {
        Self::ByResource { max_clients }
    }

    /// 创建带宽限流策略
    pub fn by_bandwidth(max_mbps: u64) -> Self {
        Self::ByBandwidth { max_mbps }
    }

    /// 创建全局限流策略
    pub fn global(max_requests: u64, window_secs: u64) -> Self {
        Self::Global {
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_strategies() {
        let ip_strategy = RateLimitStrategy::by_ip(100, 60);
        let user_strategy = RateLimitStrategy::by_user(50, 60);
        let resource_strategy = RateLimitStrategy::by_resource(1000);
        let bandwidth_strategy = RateLimitStrategy::by_bandwidth(100);
        
        match ip_strategy {
            RateLimitStrategy::ByIp { max_requests, .. } => {
                assert_eq!(max_requests, 100);
            }
            _ => panic!("Wrong strategy type"),
        }
    }
}
