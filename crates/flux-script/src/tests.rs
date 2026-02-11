#[cfg(test)]
mod script_tests {
    use crate::ScriptEngine;
    use flux_types::message::Message;
    use serde_json::json;

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
        let msg1 = Message::new("sensors/temp".to_string(), json!({ "temp": 35.0 }));
        assert!(engine.eval_message("rule_1", &msg1).unwrap());

        // Case 2: No Trigger
        let msg2 = Message::new("sensors/temp".to_string(), json!({ "temp": 20.0 }));
        assert!(!engine.eval_message("rule_1", &msg2).unwrap());
    }

    #[test]
    fn test_state_persistence() {
        let engine = ScriptEngine::new();

        let script = r#"
            let count = state_get("counter");
            if count == () {
                count = 0;
            }
            count = count + 1;
            state_set("counter", count);
            return count > 1;
        "#;

        engine.compile_script("state_rule", script).unwrap();

        // First run: count becomes 1. Returns false.
        let msg = Message::new("test".to_string(), json!({}));
        assert!(!engine.eval_message("state_rule", &msg).unwrap());

        // Second run: count becomes 2. Returns true.
        assert!(engine.eval_message("state_rule", &msg).unwrap());
    }
}
