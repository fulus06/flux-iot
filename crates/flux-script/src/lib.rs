use rhai::{Engine, Scope, AST};
use flux_types::message::Message;
use std::sync::{Arc, RwLock};

pub struct ScriptEngine {
    engine: Engine,
    // Cache compiled scripts: script_id -> AST
    script_cache: RwLock<std::collections::HashMap<String, AST>>,
    // Shared state: key -> value (used in closures registered with engine)
    #[allow(dead_code)]
    state_store: Arc<RwLock<std::collections::HashMap<String, rhai::Dynamic>>>,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        let state_store = Arc::new(RwLock::new(std::collections::HashMap::new()));
        
        // Safety: Limit max operations
        engine.set_max_operations(100_000);
        
        // Register time function
        engine.register_fn("now_ms", || {
            chrono::Utc::now().timestamp_millis()
        });

        // Register state functions via closures capturing state_store
        let store = state_store.clone();
        engine.register_fn("state_get", move |key: &str| -> rhai::Dynamic {
            // 如果锁被污染，返回 UNIT（相当于 undefined）
            match store.read() {
                Ok(read) => read.get(key).cloned().unwrap_or(rhai::Dynamic::UNIT),
                Err(_) => {
                    tracing::error!("Failed to acquire read lock for state_get");
                    rhai::Dynamic::UNIT
                }
            }
        });

        let store = state_store.clone();
        engine.register_fn("state_set", move |key: &str, value: rhai::Dynamic| {
            // 如果锁被污染，记录错误但不 panic
            match store.write() {
                Ok(mut write) => {
                    write.insert(key.to_string(), value);
                },
                Err(_) => {
                    tracing::error!("Failed to acquire write lock for state_set");
                }
            }
        });
        
        // Redirect print() to tracing::info!
        engine.on_print(|x| {
            tracing::info!("SCRIPT: {}", x);
        });
        
        Self {
            engine,
            script_cache: RwLock::new(std::collections::HashMap::new()),
            state_store,
        }
    }
    
    pub fn compile_script(&self, script_id: &str, script: &str) -> Result<(), Box<dyn std::error::Error>> {
        let ast = self.engine.compile(script)?;
        let mut cache = self.script_cache.write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        cache.insert(script_id.to_string(), ast);
        Ok(())
    }

    /// Execute a script with a Message payload.
    /// Returns true if the script evaluates to true (useful for rules).
    pub fn eval_message(&self, script_id: &str, msg: &Message) -> Result<bool, Box<dyn std::error::Error>> {
        let cache = self.script_cache.read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let ast = cache.get(script_id).ok_or("Script not found")?;

        let mut scope = Scope::new();
        
        // Inject data into scope
        // Converting to rhai::Map (which is BTreeMap<SmartString, Dynamic>)
        let payload_map = rhai::serde::to_dynamic(&msg.payload)?;
        scope.push("payload", payload_map);
        scope.push("topic", msg.topic.clone());
        scope.push("device_id", msg.id.to_string());
        
        let result: bool = self.engine.eval_ast_with_scope(&mut scope, ast)?;
        Ok(result)
    }

    pub fn get_script_ids(&self) -> Vec<String> {
        match self.script_cache.read() {
            Ok(cache) => cache.keys().cloned().collect(),
            Err(e) => {
                tracing::error!("Failed to acquire read lock in get_script_ids: {}", e);
                Vec::new()
            }
        }
    }

    pub fn remove_script(&self, id: &str) {
        match self.script_cache.write() {
            Ok(mut cache) => {
                cache.remove(id);
            },
            Err(e) => {
                tracing::error!("Failed to acquire write lock in remove_script: {}", e);
            }
        }
    }
}

mod tests;
