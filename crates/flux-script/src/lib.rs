use rhai::{Engine, Scope, AST};
use flux_types::message::Message;
use std::sync::RwLock;

pub struct ScriptEngine {
    engine: Engine,
    // Cache compiled scripts: script_id -> AST
    script_cache: RwLock<std::collections::HashMap<String, AST>>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        
        // Safety: Limit max operations to prevent infinite loops
        engine.set_max_operations(100_000);
        
        // Register types
        // Message is complex, so we might register it as a Map or specific getters
        // For simplicity, we can convert to dynamic or register methods if needed.
        // Rhai works well with serde_json::Value (Map)
        
        // Redirect print() to tracing::info!
        engine.on_print(|x| {
            tracing::info!("SCRIPT: {}", x);
        });
        
        Self {
            engine,
            script_cache: RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub fn compile_script(&self, script_id: &str, script: &str) -> Result<(), Box<dyn std::error::Error>> {
        let ast = self.engine.compile(script)?;
        self.script_cache.write().unwrap().insert(script_id.to_string(), ast);
        Ok(())
    }

    /// Execute a script with a Message payload.
    /// Returns true if the script evaluates to true (useful for rules).
    pub fn eval_message(&self, script_id: &str, msg: &Message) -> Result<bool, Box<dyn std::error::Error>> {
        let cache = self.script_cache.read().unwrap();
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
        self.script_cache.read().unwrap().keys().cloned().collect()
    }

    pub fn remove_script(&self, id: &str) {
        self.script_cache.write().unwrap().remove(id);
    }
}

mod tests;
