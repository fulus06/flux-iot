/// Macro to export the `alloc` and `dealloc` functions.
/// This is REQUIRED for the Host to write data into the Guest's linear memory.
#[macro_export]
macro_rules! export_plugin_alloc {
    () => {
        #[no_mangle]
        pub extern "C" fn alloc(len: usize) -> *mut u8 {
            let mut buf = Vec::with_capacity(len);
            let ptr = buf.as_mut_ptr();
            std::mem::forget(buf);
            ptr
        }

        #[no_mangle]
        pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }
    };
}

/// Helper to read a String passed by the Host.
/// 
/// # Safety
/// 
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that:
/// - `ptr` points to valid memory allocated by the Host
/// - The memory region `[ptr, ptr + len)` is readable
/// - The memory is valid for the lifetime of this function call
/// - `ptr` and `len` were provided by the Host through a valid function call
pub unsafe fn read_string_from_host(ptr: i32, len: i32) -> String {
    let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
    String::from_utf8_lossy(slice).to_string()
}
