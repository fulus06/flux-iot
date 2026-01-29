use rhai::{Engine, EvalAltResult};

pub struct ScriptEngine {
    engine: Engine,
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
        }
    }
}
