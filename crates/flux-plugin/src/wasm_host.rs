use anyhow::Result;
use wasmtime::{Engine, Linker, Module, Store, Config, Caller};


pub struct WasmHost {
    engine: Engine,
}

impl WasmHost {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.epoch_interruption(true); // Allow setting limits
        
        // Safety: Optimization level
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = Engine::new(&config)?;
        Ok(Self { engine })
    }

    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<Module> {
        Module::new(&self.engine, wasm_bytes)
    }
    
    /// prepare a fresh store for a new instance
    pub fn create_store(&self) -> Store<()> {
        Store::new(&self.engine, ())
    }

    /// Linker with default imports (logging, etc)
    pub fn create_linker(&self) -> Linker<()> {
        let mut linker = Linker::new(&self.engine);
        
        // 注册多级别日志函数
        Self::register_log_functions(&mut linker);
        
        linker
    }
    
    /// 注册所有日志级别的导入函数
    fn register_log_functions(linker: &mut Linker<()>) {
        // 使用宏减少重复代码
        macro_rules! register_log {
            ($name:literal, $level:expr) => {
                if let Err(e) = linker.func_wrap(
                    "env",
                    $name,
                    move |mut caller: Caller<'_, ()>, ptr: i32, len: i32| {
                        Self::handle_log(&mut caller, ptr, len, $level);
                    }
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
    fn handle_log(caller: &mut Caller<'_, ()>, ptr: i32, len: i32, level: tracing::Level) {
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
                    ptr, len, data.len()
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
            },
            Err(e) => {
                tracing::warn!("Invalid UTF-8 in plugin log (len={}): {}", len, e);
            }
        }
    }
}
