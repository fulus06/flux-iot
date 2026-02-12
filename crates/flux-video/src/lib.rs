// flux-video: 视频流监控核心库
// 
// 架构设计：
// - 极致轻量：单节点模式 40-80MB 内存
// - 高并发：支持 100+ 路摄像头
// - 双模式：单节点/分布式灵活切换

pub mod error;
pub mod engine;
pub mod stream;
pub mod codec;
pub mod storage;
pub mod snapshot;
pub mod ai;
pub mod metrics;

// GB28181 协议（独立模块）
pub mod gb28181;

// 重新导出常用类型
pub use error::{VideoError, Result};

/// 初始化视频模块
pub fn init() {
    tracing::info!("flux-video initialized");
}
