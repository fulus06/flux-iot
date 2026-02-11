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
        
        // 获取写锁，如果锁被污染则返回错误
        let mut instances = self.instances.write()
            .map_err(|e| anyhow!("Failed to acquire write lock: {}", e))?;
        instances.insert(plugin_id.to_string(), plugin_instance);
        Ok(())
    }

    /// Call a function in the plugin.
    /// Example: "on_msg(ptr, len) -> int"
    pub fn call_plugin(&self, plugin_id: &str, function_name: &str, input_data: &str) -> Result<i32> {
        let mut map = self.instances.write()
            .map_err(|e| anyhow!("Failed to acquire write lock: {}", e))?;
        let plugin = map.get_mut(plugin_id)
            .ok_or_else(|| anyhow!("Plugin not found: {}", plugin_id))?;
            
        let instance = plugin.instance;
        let mut store = &mut plugin.store;

        // 1. Get exports
        let alloc_fn = instance.get_typed_func::<i32, i32>(&mut store, "alloc")
            .context("Plugin must export 'alloc' function")?;
        
        let dealloc_fn = instance.get_typed_func::<(i32, i32), ()>(&mut store, "dealloc")
            .context("Plugin must export 'dealloc' function")?;
        
        let target_fn = instance.get_typed_func::<(i32, i32), i32>(&mut store, function_name)
            .context(format!("Plugin must export '{}' function", function_name))?;
            
        let memory = instance.get_memory(&mut store, "memory")
            .context("Plugin must export 'memory'")?;

        // 2. Write input to Wasm memory
        let bytes = input_data.as_bytes();
        let len = bytes.len() as i32;
        let ptr = alloc_fn.call(&mut store, len)?;
        
        // 确保即使写入失败也能释放内存
        if let Err(e) = memory.write(&mut store, ptr as usize, bytes) {
            let _ = dealloc_fn.call(&mut store, (ptr, len));
            return Err(e.into());
        }
        
        // 3. Call the function
        let result = target_fn.call(&mut store, (ptr, len));
        
        // 4. 释放 Wasm 内存（无论函数调用成功与否）
        // Safety: 必须在使用完内存后立即释放，防止内存泄漏
        let dealloc_result = dealloc_fn.call(&mut store, (ptr, len));
        
        // 优先返回业务逻辑错误，但记录 dealloc 失败
        match (result, dealloc_result) {
            (Ok(r), Ok(())) => Ok(r),
            (Ok(_), Err(e)) => {
                tracing::error!("Failed to deallocate Wasm memory: {}", e);
                Err(anyhow!("Memory deallocation failed: {}", e))
            },
            (Err(e), Ok(())) => Err(e),
            (Err(e1), Err(e2)) => {
                tracing::error!("Failed to deallocate Wasm memory: {}", e2);
                Err(anyhow!("Plugin call failed: {}, dealloc also failed: {}", e1, e2))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 获取测试用的 dummy_plugin.wasm
    fn get_test_plugin_bytes() -> Result<Vec<u8>> {
        let plugin_path = std::env::current_dir()?
            .join("plugins")
            .join("dummy_plugin.wasm");
        
        std::fs::read(&plugin_path)
            .context(format!("Failed to read test plugin from {:?}", plugin_path))
    }

    #[test]
    fn test_plugin_manager_new() {
        let manager = PluginManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_load_plugin_success() {
        let manager = PluginManager::new().unwrap();
        let wasm_bytes = match get_test_plugin_bytes() {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("Skipping test: dummy_plugin.wasm not found");
                return;
            }
        };

        let result = manager.load_plugin("test_plugin", &wasm_bytes);
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_invalid_wasm() {
        let manager = PluginManager::new().unwrap();
        let invalid_wasm = vec![0x00, 0x61, 0x73, 0x6d]; // 不完整的 Wasm 头

        let result = manager.load_plugin("invalid", &invalid_wasm);
        assert!(result.is_err());
    }

    #[test]
    fn test_call_plugin_success() {
        let manager = PluginManager::new().unwrap();
        let wasm_bytes = match get_test_plugin_bytes() {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("Skipping test: dummy_plugin.wasm not found");
                return;
            }
        };

        manager.load_plugin("dummy", &wasm_bytes).unwrap();

        // 调用插件的 on_msg 函数
        let input = r#"{"topic":"test","payload":{"value":42}}"#;
        let result = manager.call_plugin("dummy", "on_msg", input);
        
        assert!(result.is_ok());
        // dummy_plugin 返回输入字符串的长度
        assert_eq!(result.unwrap(), input.len() as i32);
    }

    #[test]
    fn test_call_nonexistent_plugin() {
        let manager = PluginManager::new().unwrap();

        let result = manager.call_plugin("nonexistent", "on_msg", "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plugin not found"));
    }

    #[test]
    fn test_call_nonexistent_function() {
        let manager = PluginManager::new().unwrap();
        let wasm_bytes = match get_test_plugin_bytes() {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("Skipping test: dummy_plugin.wasm not found");
                return;
            }
        };

        manager.load_plugin("dummy", &wasm_bytes).unwrap();

        let result = manager.call_plugin("dummy", "nonexistent_func", "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must export"));
    }

    #[test]
    fn test_call_plugin_with_empty_input() {
        let manager = PluginManager::new().unwrap();
        let wasm_bytes = match get_test_plugin_bytes() {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("Skipping test: dummy_plugin.wasm not found");
                return;
            }
        };

        manager.load_plugin("dummy", &wasm_bytes).unwrap();

        let result = manager.call_plugin("dummy", "on_msg", "");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // 空字符串长度为 0
    }

    #[test]
    fn test_call_plugin_with_large_input() {
        let manager = PluginManager::new().unwrap();
        let wasm_bytes = match get_test_plugin_bytes() {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("Skipping test: dummy_plugin.wasm not found");
                return;
            }
        };

        manager.load_plugin("dummy", &wasm_bytes).unwrap();

        // 创建一个较大的输入（10KB）
        let large_input = "x".repeat(10240);
        let result = manager.call_plugin("dummy", "on_msg", &large_input);
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10240);
    }

    #[test]
    fn test_load_multiple_plugins() {
        let manager = PluginManager::new().unwrap();
        let wasm_bytes = match get_test_plugin_bytes() {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("Skipping test: dummy_plugin.wasm not found");
                return;
            }
        };

        // 加载同一个插件的多个实例
        manager.load_plugin("plugin1", &wasm_bytes).unwrap();
        manager.load_plugin("plugin2", &wasm_bytes).unwrap();
        manager.load_plugin("plugin3", &wasm_bytes).unwrap();

        // 所有插件都应该可以独立调用
        assert!(manager.call_plugin("plugin1", "on_msg", "test1").is_ok());
        assert!(manager.call_plugin("plugin2", "on_msg", "test2").is_ok());
        assert!(manager.call_plugin("plugin3", "on_msg", "test3").is_ok());
    }

    #[test]
    fn test_reload_plugin() {
        let manager = PluginManager::new().unwrap();
        let wasm_bytes = match get_test_plugin_bytes() {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("Skipping test: dummy_plugin.wasm not found");
                return;
            }
        };

        // 加载插件
        manager.load_plugin("reload_test", &wasm_bytes).unwrap();
        
        // 重新加载同一个插件（应该覆盖）
        let result = manager.load_plugin("reload_test", &wasm_bytes);
        assert!(result.is_ok());

        // 插件应该仍然可以调用
        assert!(manager.call_plugin("reload_test", "on_msg", "test").is_ok());
    }
}
