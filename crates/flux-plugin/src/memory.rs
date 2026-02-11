use anyhow::{anyhow, Result};
use wasmtime::{Caller, Memory};

/// Read a string from Wasm memory.
/// Ptr and Len are passed from Wasm.
pub fn read_string_from_wasm(
    memory: &Memory,
    caller: &mut Caller<'_, ()>,
    ptr: i32,
    len: i32,
) -> Result<String> {
    let data = memory
        .data(caller)
        .get(ptr as usize..(ptr + len) as usize)
        .ok_or(anyhow!("Pointer/Length out of bounds"))?;

    Ok(String::from_utf8(data.to_vec())?)
}

/// Write a byte slice to Wasm memory.
/// Returns the pointer to the written data.
/// WARNING: This assumes the Wasm module has exported "alloc" function to allocate memory.
/// This function is usually called by the Host wrapper, not directly inside imports.
pub fn write_bytes_to_wasm(
    memory: &Memory,
    caller: &mut Caller<'_, ()>,
    alloc_fn: &wasmtime::TypedFunc<i32, i32>,
    bytes: &[u8],
) -> Result<(i32, i32)> {
    let len = bytes.len() as i32;
    let ptr = alloc_fn.call(&mut *caller, len)?;

    memory.write(&mut *caller, ptr as usize, bytes)?;

    Ok((ptr, len))
}
