#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScriptEngine;
    use serde_json::json;
    use flux_types::message::Message;

    #[test]
    fn test_eval_rule() {
        let engine = ScriptEngine::new();
        
        let script = r#"
            // Rhai script
            if payload.temp > 30.0 {
                return true;
            } else {
                return false;
            }
        "#;
        
        engine.compile_script("rule_1", script).unwrap();
        
        // Case 1: Trigger
        let msg1 = Message::new("sensors/temp", json!({ "temp": 35.0 }));
        assert_eq!(engine.eval_message("rule_1", &msg1).unwrap(), true);

        // Case 2: No Trigger
        let msg2 = Message::new("sensors/temp", json!({ "temp": 20.0 }));
        assert_eq!(engine.eval_message("rule_1", &msg2).unwrap(), false);
    }
}
