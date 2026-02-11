use flux_plugin_sdk::{export_plugin_alloc, read_string_from_host, trace, debug, info, warn, error};

export_plugin_alloc!();

/// 插件入口函数：处理来自 Host 的消息
/// 
/// 演示多级别日志的使用场景
#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    trace!("on_msg called with ptr={}, len={}", ptr, len);
    
    // 读取输入字符串
    let input = unsafe { read_string_from_host(ptr, len) };
    debug!("Received message: {} bytes", input.len());
    
    // 检查空消息
    if input.is_empty() {
        warn!("Empty message received, ignoring");
        return 0;
    }
    
    // 模拟数据处理
    if input.len() > 1024 {
        warn!("Large message received: {} bytes (threshold: 1024)", input.len());
    }
    
    // 模拟 JSON 解析
    if input.starts_with('{') && input.ends_with('}') {
        info!("Processing JSON message");
        
        // 简单的温度检测逻辑（模拟）
        if input.contains("temperature") {
            if input.contains("95") || input.contains("100") {
                error!("Critical temperature detected in message!");
            } else if input.contains("85") || input.contains("90") {
                warn!("High temperature detected in message");
            } else {
                debug!("Normal temperature reading");
            }
        }
    } else {
        debug!("Processing plain text message");
    }
    
    info!("Message processed successfully, length: {}", input.len());
    trace!("Returning result: {}", input.len());
    
    input.len() as i32
}
