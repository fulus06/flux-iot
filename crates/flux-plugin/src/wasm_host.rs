use anyhow::{Result, Context};
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
        
        // Example: Import a logging function
        linker.func_wrap("env", "log_info", |caller: Caller<'_, ()>, ptr: i32, len: i32| {
           // TODO: Read string and log it
           // let mem = match caller.get_export("memory") ...
           println!("Log from wasm: ptr={}, len={}", ptr, len);
        }).unwrap();
        
        linker
    }
}
