use flux_script::ScriptEngine;

#[tokio::test]
async fn test_date_add_function() {
    let mut script_engine = ScriptEngine::new();
    flux_rule::functions::register_builtin_functions(&mut script_engine);

    let script = r#"
        let now = now();
        let tomorrow = date_add(now, 1, "days");
        let next_week = date_add(now, 7, "days");
        let next_hour = date_add(now, 1, "hours");
        
        // 验证时间戳增加
        tomorrow.timestamp > now.timestamp
    "#;

    let result = script_engine.eval(script).unwrap();
    assert!(result.as_bool().unwrap());
}

#[tokio::test]
async fn test_format_date_function() {
    let mut script_engine = ScriptEngine::new();
    flux_rule::functions::register_builtin_functions(&mut script_engine);

    let script = r#"
        let now = now();
        let formatted = format_date(now, "%Y-%m-%d");
        
        // 验证格式化结果包含年份
        formatted.len() == 10
    "#;

    let result = script_engine.eval(script).unwrap();
    assert!(result.as_bool().unwrap());
}

#[tokio::test]
async fn test_date_start_of_day() {
    let mut script_engine = ScriptEngine::new();
    flux_rule::functions::register_builtin_functions(&mut script_engine);

    let script = r#"
        let now = now();
        let start = date_start_of_day(now);
        
        // 验证小时和分钟都是0
        start.hour == 0 && start.minute == 0
    "#;

    let result = script_engine.eval(script).unwrap();
    assert!(result.as_bool().unwrap());
}

#[tokio::test]
async fn test_date_end_of_day() {
    let mut script_engine = ScriptEngine::new();
    flux_rule::functions::register_builtin_functions(&mut script_engine);

    let script = r#"
        let now = now();
        let end = date_end_of_day(now);
        
        // 验证小时是23，分钟是59
        end.hour == 23 && end.minute == 59
    "#;

    let result = script_engine.eval(script).unwrap();
    assert!(result.as_bool().unwrap());
}

#[tokio::test]
async fn test_date_manipulation_workflow() {
    let mut script_engine = ScriptEngine::new();
    flux_rule::functions::register_builtin_functions(&mut script_engine);

    let script = r#"
        // 获取今天开始时间
        let today_start = date_start_of_day(now());
        
        // 获取明天开始时间
        let tomorrow = date_add(today_start, 1, "days");
        
        // 格式化输出
        let formatted = format_date(tomorrow, "%Y-%m-%d %H:%M:%S");
        
        // 验证明天的时间戳大于今天
        tomorrow.timestamp > today_start.timestamp
    "#;

    let result = script_engine.eval(script).unwrap();
    assert!(result.as_bool().unwrap());
}
