//! Wasm 插件日志功能
//! 
//! 提供多级别日志宏，与 Host 的 tracing 系统集成

// Host 提供的日志函数声明
// 注意：必须声明为 pub，因为宏会在其他 crate 中展开并调用这些函数
extern "C" {
    pub fn log_trace(ptr: *const u8, len: usize);
    pub fn log_debug(ptr: *const u8, len: usize);
    pub fn log_info(ptr: *const u8, len: usize);
    pub fn log_warn(ptr: *const u8, len: usize);
    pub fn log_error(ptr: *const u8, len: usize);
}

/// TRACE 级别日志宏
/// 
/// 用于非常详细的追踪信息，通常只在开发环境启用
/// 
/// # 示例
/// ```ignore
/// trace!("Function called with param: {}", value);
/// ```
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        unsafe {
            $crate::logging::log_trace(msg.as_ptr(), msg.len());
        }
    }};
}

/// DEBUG 级别日志宏
/// 
/// 用于调试信息，帮助开发者理解程序执行流程
/// 
/// # 示例
/// ```ignore
/// debug!("Processing data: {:?}", data);
/// ```
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        unsafe {
            $crate::logging::log_debug(msg.as_ptr(), msg.len());
        }
    }};
}

/// INFO 级别日志宏
/// 
/// 用于正常的运行时信息，记录重要的业务事件
/// 
/// # 示例
/// ```ignore
/// info!("Device connected: {}", device_id);
/// ```
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        unsafe {
            $crate::logging::log_info(msg.as_ptr(), msg.len());
        }
    }};
}

/// WARN 级别日志宏
/// 
/// 用于警告信息，表示潜在问题但不影响正常运行
/// 
/// # 示例
/// ```ignore
/// warn!("Temperature high: {}°C", temp);
/// ```
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        unsafe {
            $crate::logging::log_warn(msg.as_ptr(), msg.len());
        }
    }};
}

/// ERROR 级别日志宏
/// 
/// 用于错误信息，表示严重问题需要关注
/// 
/// # 示例
/// ```ignore
/// error!("Failed to parse data: {}", err);
/// ```
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        unsafe {
            $crate::logging::log_error(msg.as_ptr(), msg.len());
        }
    }};
}
