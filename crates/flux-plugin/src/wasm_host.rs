use anyhow::Result;
use wasmtime::{Caller, Config, Engine, Linker, Module, ResourceLimiter, Store};

/// Wasm 资源限制配置
#[derive(Debug, Clone)]
pub struct WasmResourceLimits {
    /// 最大内存大小（字节）
    pub max_memory_bytes: usize,
    /// 最大表大小（元素数量）
    pub max_table_elements: u32,
    /// 最大实例数量
    pub max_instances: usize,
    /// 最大表数量
    pub max_tables: usize,
    /// 最大内存数量
    pub max_memories: usize,
}

impl Default for WasmResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024,  // 16 MB
            max_table_elements: 1000,
            max_instances: 10,
            max_tables: 1,
            max_memories: 1,
        }
    }
}

/// 资源限制器实现
pub struct WasmResourceLimiter {
    max_memory_bytes: usize,
    max_table_elements: u32,
}

impl WasmResourceLimiter {
    fn new(config: &WasmResourceLimits) -> Self {
        Self {
            max_memory_bytes: config.max_memory_bytes,
            max_table_elements: config.max_table_elements,
        }
    }
}

impl ResourceLimiter for WasmResourceLimiter {
    fn memory_growing(&mut self, current: usize, desired: usize, _maximum: Option<usize>) -> anyhow::Result<bool> {
        let allowed = desired <= self.max_memory_bytes;
        if !allowed {
            tracing::warn!(
                "Wasm memory limit exceeded: current={}, desired={}, max={}",
                current, desired, self.max_memory_bytes
            );
        }
        Ok(allowed)
    }

    fn table_growing(&mut self, current: u32, desired: u32, _maximum: Option<u32>) -> anyhow::Result<bool> {
        let allowed = desired <= self.max_table_elements;
        if !allowed {
            tracing::warn!(
                "Wasm table limit exceeded: current={}, desired={}, max={}",
                current, desired, self.max_table_elements
            );
        }
        Ok(allowed)
    }
}

pub struct WasmHost {
    engine: Engine,
    resource_limits: WasmResourceLimits,
}

impl WasmHost {
    pub fn new() -> Result<Self> {
        Self::with_limits(WasmResourceLimits::default())
    }
    
    pub fn with_limits(resource_limits: WasmResourceLimits) -> Result<Self> {
        let mut config = Config::new();
        
        // 启用 epoch 中断（用于 CPU 时间限制）
        config.epoch_interruption(true);
        
        // 启用资源限制
        config.consume_fuel(true);
        
        // 优化级别
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = Engine::new(&config)?;
        Ok(Self { engine, resource_limits })
    }

    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<Module> {
        Module::new(&self.engine, wasm_bytes)
    }

    /// 创建带资源限制的 Store
    pub fn create_store(&self) -> Store<WasmResourceLimiter> {
        let limiter = WasmResourceLimiter::new(&self.resource_limits);
        let mut store = Store::new(&self.engine, limiter);
        
        // 设置资源限制器
        store.limiter(|limiter| limiter);
        
        // 注意：fuel 功能需要在编译时启用特定特性
        // 这里我们主要依赖 ResourceLimiter 来限制内存和表大小
        // CPU 时间限制可以通过 epoch interruption 实现
        
        store
    }

    /// Linker with default imports (logging, etc)
    pub fn create_linker(&self) -> Linker<WasmResourceLimiter> {
        let mut linker = Linker::new(&self.engine);

        // 注册多级别日志函数
        Self::register_log_functions(&mut linker);

        linker
    }

    /// 注册所有日志级别的导入函数
    fn register_log_functions(linker: &mut Linker<WasmResourceLimiter>) {
        // 使用宏减少重复代码
        macro_rules! register_log {
            ($name:literal, $level:expr) => {
                if let Err(e) = linker.func_wrap(
                    "env",
                    $name,
                    move |mut caller: Caller<'_, WasmResourceLimiter>, ptr: i32, len: i32| {
                        Self::handle_log(&mut caller, ptr, len, $level);
                    },
                ) {
                    tracing::error!("Failed to register {}: {}", $name, e);
                }
            };
        }

        register_log!("log_trace", tracing::Level::TRACE);
        register_log!("log_debug", tracing::Level::DEBUG);
        register_log!("log_info", tracing::Level::INFO);
        register_log!("log_warn", tracing::Level::WARN);
        register_log!("log_error", tracing::Level::ERROR);
    }

    /// 处理来自 Wasm 插件的日志调用
    fn handle_log(caller: &mut Caller<'_, WasmResourceLimiter>, ptr: i32, len: i32, level: tracing::Level) {
        const MAX_LOG_LEN: usize = 4096; // 防止超大日志

        // 1. 获取 Wasm 线性内存
        let memory = match caller.get_export("memory") {
            Some(wasmtime::Extern::Memory(mem)) => mem,
            _ => {
                tracing::error!("Wasm plugin: failed to get memory export");
                return;
            }
        };

        // 2. 限制日志长度
        let len = (len as usize).min(MAX_LOG_LEN);
        if len == 0 {
            return;
        }

        // 3. 读取内存中的字符串
        let data = memory.data(caller);
        let slice = match data.get(ptr as usize..(ptr as usize + len)) {
            Some(s) => s,
            None => {
                tracing::error!(
                    "Invalid memory range in plugin log: ptr={}, len={}, memory_size={}",
                    ptr,
                    len,
                    data.len()
                );
                return;
            }
        };

        // 4. 验证 UTF-8 并输出日志
        match std::str::from_utf8(slice) {
            Ok(msg) => {
                // 根据级别输出到 tracing 系统
                match level {
                    tracing::Level::TRACE => tracing::trace!(target: "wasm_plugin", "{}", msg),
                    tracing::Level::DEBUG => tracing::debug!(target: "wasm_plugin", "{}", msg),
                    tracing::Level::INFO => tracing::info!(target: "wasm_plugin", "{}", msg),
                    tracing::Level::WARN => tracing::warn!(target: "wasm_plugin", "{}", msg),
                    tracing::Level::ERROR => tracing::error!(target: "wasm_plugin", "{}", msg),
                }
            }
            Err(e) => {
                tracing::warn!("Invalid UTF-8 in plugin log (len={}): {}", len, e);
            }
        }
    }
}
