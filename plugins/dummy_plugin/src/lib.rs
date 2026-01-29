use flux_plugin_sdk::export_plugin_alloc;

export_plugin_alloc!();

#[no_mangle]
pub extern "C" fn on_msg(ptr: i32, len: i32) -> i32 {
    // 1. Read input from Host
    let input_str = unsafe { flux_plugin_sdk::read_string_from_host(ptr, len) };
    
    // 2. Log (Simulate)
    // In a real plugin, we would call an imported 'log' function.
    // Here we just return the length of the string to prove we read it.
    
    // 3. Simple logic: return length as dummy result
    input_str.len() as i32
}
