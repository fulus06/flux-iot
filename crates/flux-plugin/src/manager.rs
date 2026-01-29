use crate::wasm_host::WasmHost;
use anyhow::{Result, Context, anyhow};
use std::collections::HashMap;
use std::sync::RwLock;
use wasmtime::Store;

pub struct PluginManager {
    host: WasmHost,
    // Store instances for each plugin.
    // In a real scenario, we might want to recycle instances or create them on demand.
    // For now, we keep one instance per plugin for simplicity.
    instances: RwLock<HashMap<String, PluginInstance>>,
}

struct PluginInstance {
    store: Store<()>,
    instance: wasmtime::Instance,
}

impl PluginManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            host: WasmHost::new()?,
            instances: RwLock::new(HashMap::new()),
        })
    }

    pub fn load_plugin(&self, plugin_id: &str, wasm_bytes: &[u8]) -> Result<()> {
        let module = self.host.load_module(wasm_bytes)?;
        let linker = self.host.create_linker();
        let mut store = self.host.create_store();
        
        let instance = linker.instantiate(&mut store, &module)
            .context("Failed to instantiate plugin")?;
        
        let plugin_instance = PluginInstance {
            store,
            instance,
        };
        
        self.instances.write().unwrap().insert(plugin_id.to_string(), plugin_instance);
        Ok(())
    }

    /// Call a function in the plugin.
    /// Example: "on_msg(ptr, len) -> int"
    pub fn call_plugin(&self, plugin_id: &str, function_name: &str, input_data: &str) -> Result<i32> {
        let mut map = self.instances.write().unwrap();
        let plugin = map.get_mut(plugin_id)
            .ok_or_else(|| anyhow!("Plugin not found: {}", plugin_id))?;
            
        let instance = plugin.instance;
        let mut store = &mut plugin.store;

        // 1. Get exports
        let alloc_fn = instance.get_typed_func::<i32, i32>(&mut store, "alloc")
            .context("Plugin must export 'alloc' function")?;
        
        let target_fn = instance.get_typed_func::<(i32, i32), i32>(&mut store, function_name)
            .context(format!("Plugin must export '{}' function", function_name))?;
            
        let memory = instance.get_memory(&mut store, "memory")
            .context("Plugin must export 'memory'")?;

        // 2. Write input to Wasm memory
        // We use our helper from memory.rs (requires passing memory, caller, alloc_fn, bytes)
        // Since we are outside of a host call, we interact with store directly.
        let bytes = input_data.as_bytes();
        let ptr = alloc_fn.call(&mut store, bytes.len() as i32)?;
        memory.write(&mut store, ptr as usize, bytes)?;
        
        // 3. Call the function
        let result = target_fn.call(&mut store, (ptr, bytes.len() as i32))?;
        
        // 4. (Optional) Dealloc
        // Ideally we should call dealloc. Leaving it for now.
        
        Ok(result)
    }
}
