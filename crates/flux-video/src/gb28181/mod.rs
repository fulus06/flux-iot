// GB28181 国标协议（独立模块）
// 
// 架构：
// - sip/: SIP 信令层
// - rtp/: RTP 传输层
// - ps/: PS 流解封装层
// - stream/: GB28181 协议实现

pub mod sip;
pub mod rtp;
pub mod ps;
pub mod stream;

pub use stream::{Gb28181Stream, Gb28181StreamConfig};
