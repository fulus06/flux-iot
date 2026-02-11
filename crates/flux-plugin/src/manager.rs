use crate::wasm_host::{WasmHost, WasmResourceLimiter};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wasmtime::{Module, Store};

/// 插件实例池配置
const DEFAULT_POOL_SIZE: usize = 4; // 每个插件默认保持4个实例

pub struct PluginManager {
    host: WasmHost,
    // 存储每个插件的模块和实例池
    plugins: RwLock<HashMap<String, PluginPool>>,
    pool_size: usize,
}

/// 单个插件的实例池
struct PluginPool {
    module: Arc<Module>,
    // 可用实例队列（使用 Vec 作为简单的池）
    available: Vec<PluginInstance>,
    // 实例池大小限制
    max_size: usize,
}

struct PluginInstance {
    store: Store<WasmResourceLimiter>,
    instance: wasmtime::Instance,
}

impl PluginManager {
    pub fn new() -> Result<Self> {
        Self::with_pool_size(DEFAULT_POOL_SIZE)
    }

    pub fn with_pool_size(pool_size: usize) -> Result<Self> {
        Ok(Self {
            host: WasmHost::new()?,
            plugins: RwLock::new(HashMap::new()),
            pool_size,
        })
    }

    pub fn load_plugin(&self, plugin_id: &str, wasm_bytes: &[u8]) -> Result<()> {
        let module = Arc::new(self.host.load_module(wasm_bytes)?);

        // 预创建初始实例（懒加载策略：先创建1个，按需增长）
        let linker = self.host.create_linker();
        let mut store = self.host.create_store();
        let instance = linker
            .instantiate(&mut store, &module)
            .context("Failed to instantiate plugin")?;

        let plugin_instance = PluginInstance { store, instance };

        let pool = PluginPool {
            module,
            available: vec![plugin_instance],
            max_size: self.pool_size,
        };

        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| anyhow!("Failed to acquire write lock: {}", e))?;
        plugins.insert(plugin_id.to_string(), pool);

        tracing::debug!(
            "Loaded plugin '{}' with pool size {}",
            plugin_id,
            self.pool_size
        );
        Ok(())
    }

    /// Call a function in the plugin.
    /// Example: "on_msg(ptr, len) -> int"
    /// 使用实例池策略：从池中获取实例，使用后归还
    pub fn call_plugin(
        &self,
        plugin_id: &str,
        function_name: &str,
        input_data: &str,
    ) -> Result<i32> {
        // 1. 从池中获取或创建实例
        let mut instance = self.acquire_instance(plugin_id)?;

        // 2. 执行插件调用
        let result = self.execute_plugin_call(&mut instance, function_name, input_data);

        // 3. 归还实例到池中（无论成功失败）
        self.release_instance(plugin_id, instance)?;

        result
    }

    /// 从实例池获取一个可用实例
    fn acquire_instance(&self, plugin_id: &str) -> Result<PluginInstance> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| anyhow!("Failed to acquire write lock: {}", e))?;

        let pool = plugins
            .get_mut(plugin_id)
            .ok_or_else(|| anyhow!("Plugin not found: {}", plugin_id))?;

        // 尝试从池中获取实例
        if let Some(instance) = pool.available.pop() {
            tracing::trace!(
                "Reusing instance from pool for plugin '{}' (pool hit)",
                plugin_id
            );
            // Metrics: 池命中
            return Ok(instance);
        }

        // 池为空，创建新实例（如果未达到上限）
        tracing::debug!(
            "Creating new instance for plugin '{}' (pool miss)",
            plugin_id
        );
        // Metrics: 池未命中

        if pool.available.len() < pool.max_size {
            let linker = self.host.create_linker();
            let mut store = self.host.create_store();
            let instance = linker
                .instantiate(&mut store, &pool.module)
                .context("Failed to instantiate plugin from pool")?;

            Ok(PluginInstance { store, instance })
        } else {
            // 池已满且无可用实例，创建临时实例
            tracing::warn!(
                "Plugin '{}' pool exhausted, creating temporary instance",
                plugin_id
            );
            let linker = self.host.create_linker();
            let mut store = self.host.create_store();
            let instance = linker
                .instantiate(&mut store, &pool.module)
                .context("Failed to instantiate temporary plugin instance")?;

            Ok(PluginInstance { store, instance })
        }
    }

    /// 归还实例到池中
    fn release_instance(&self, plugin_id: &str, instance: PluginInstance) -> Result<()> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| anyhow!("Failed to acquire write lock: {}", e))?;

        if let Some(pool) = plugins.get_mut(plugin_id) {
            // 只有池未满时才归还
            if pool.available.len() < pool.max_size {
                pool.available.push(instance);
                tracing::trace!("Returned instance to pool for plugin '{}'", plugin_id);
            } else {
                // 池已满，丢弃实例（自动清理）
                tracing::trace!("Pool full, discarding instance for plugin '{}'", plugin_id);
            }
        }

        Ok(())
    }

    /// 执行实际的插件调用
    fn execute_plugin_call(
        &self,
        plugin: &mut PluginInstance,
        function_name: &str,
        input_data: &str,
    ) -> Result<i32> {
        let instance = plugin.instance;
        let store = &mut plugin.store;

        // 1. Get exports
        let alloc_fn = instance
            .get_typed_func::<i32, i32>(&mut *store, "alloc")
            .context("Plugin must export 'alloc' function")?;

        let dealloc_fn = instance
            .get_typed_func::<(i32, i32), ()>(&mut *store, "dealloc")
            .context("Plugin must export 'dealloc' function")?;

        let target_fn = instance
            .get_typed_func::<(i32, i32), i32>(&mut *store, function_name)
            .context(format!("Plugin must export '{}' function", function_name))?;

        let memory = instance
            .get_memory(&mut *store, "memory")
            .context("Plugin must export 'memory'")?;

        // 2. Write input to Wasm memory
        let bytes = input_data.as_bytes();
        let len = bytes.len() as i32;
        let ptr = alloc_fn.call(&mut *store, len)?;

        // 确保即使写入失败也能释放内存
        if let Err(e) = memory.write(&mut *store, ptr as usize, bytes) {
            let _ = dealloc_fn.call(&mut *store, (ptr, len));
            return Err(e.into());
        }

        // 3. Call the function
        let result = target_fn.call(&mut *store, (ptr, len));

        // 4. 释放 Wasm 内存（无论函数调用成功与否）
        let dealloc_result = dealloc_fn.call(&mut *store, (ptr, len));

        // 优先返回业务逻辑错误，但记录 dealloc 失败
        match (result, dealloc_result) {
            (Ok(r), Ok(())) => Ok(r),
            (Ok(_), Err(e)) => {
                tracing::error!("Failed to deallocate Wasm memory: {}", e);
                Err(anyhow!("Memory deallocation failed: {}", e))
            }
            (Err(e), Ok(())) => Err(e),
            (Err(e1), Err(e2)) => {
                tracing::error!("Failed to deallocate Wasm memory: {}", e2);
                Err(anyhow!(
                    "Plugin call failed: {}, dealloc also failed: {}",
                    e1,
                    e2
                ))
            }
        }
    }

    /// 获取插件池统计信息
    pub fn get_pool_stats(&self, plugin_id: &str) -> Result<PoolStats> {
        let plugins = self
            .plugins
            .read()
            .map_err(|e| anyhow!("Failed to acquire read lock: {}", e))?;

        let pool = plugins
            .get(plugin_id)
            .ok_or_else(|| anyhow!("Plugin not found: {}", plugin_id))?;

        Ok(PoolStats {
            available: pool.available.len(),
            max_size: pool.max_size,
        })
    }
}

/// 插件池统计信息
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub available: usize,
    pub max_size: usize,
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
