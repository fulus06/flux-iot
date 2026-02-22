use flux_script::ScriptEngine;
use tracing::{debug, error, info, warn};

/// 注册所有内置函数
pub fn register_builtin_functions(engine: &mut ScriptEngine) {
    let rhai_engine = engine.engine_mut();
    register_device_functions(rhai_engine);
    register_notification_functions(rhai_engine);
    register_data_functions(rhai_engine);
    register_time_functions(rhai_engine);
    register_log_functions(rhai_engine);
}

/// 注册设备控制函数
fn register_device_functions(engine: &mut rhai::Engine) {
    // control_device(device_id, command, params)
    engine.register_fn("control_device", |device_id: &str, command: &str, params: rhai::Map| {
        info!(
            device_id = %device_id,
            command = %command,
            "Control device (mock)"
        );
        // TODO: 实际实现需要调用设备控制服务
        debug!("Params: {:?}", params);
    });
    
    // read_device(device_id, metric)
    engine.register_fn("read_device", |device_id: &str, metric: &str| -> rhai::Dynamic {
        debug!(device_id = %device_id, metric = %metric, "Read device (mock)");
        // TODO: 实际实现需要调用设备读取服务
        rhai::Dynamic::from(0.0)
    });
    
    // update_device_status(device_id, status)
    engine.register_fn("update_device_status", |device_id: &str, status: &str| {
        info!(device_id = %device_id, status = %status, "Update device status (mock)");
        // TODO: 实际实现需要调用设备状态更新服务
    });
}

/// 注册通知函数
fn register_notification_functions(engine: &mut rhai::Engine) {
    // send_notification(channel, title, message)
    engine.register_fn("send_notification", |channel: &str, title: &str, message: &str| {
        info!(
            channel = %channel,
            title = %title,
            message = %message,
            "Send notification (mock)"
        );
        // TODO: 实际实现需要调用通知服务
    });
    
    // send_email(to, subject, body)
    engine.register_fn("send_email", |params: rhai::Map| {
        info!("Send email (mock): {:?}", params);
        // TODO: 实际实现需要调用邮件服务
    });
    
    // send_sms(phone, message)
    engine.register_fn("send_sms", |phone: &str, message: &str| {
        info!(phone = %phone, message = %message, "Send SMS (mock)");
        // TODO: 实际实现需要调用短信服务
    });
    
    // send_push(user_id, title, message)
    engine.register_fn("send_push", |user_id: &str, title: &str, message: &str| {
        info!(
            user_id = %user_id,
            title = %title,
            message = %message,
            "Send push notification (mock)"
        );
        // TODO: 实际实现需要调用推送服务
    });
}

/// 注册数据查询函数
fn register_data_functions(engine: &mut rhai::Engine) {
    // query_metrics(params)
    engine.register_fn("query_metrics", |params: rhai::Map| -> rhai::Map {
        debug!("Query metrics (mock): {:?}", params);
        // TODO: 实际实现需要调用数据查询服务
        let mut result = rhai::Map::new();
        result.insert("total".into(), rhai::Dynamic::from(100.0));
        result.insert("average".into(), rhai::Dynamic::from(50.0));
        result.insert("peak".into(), rhai::Dynamic::from(80.0));
        result
    });
    
    // count_events(event_type, time_range)
    engine.register_fn("count_events", |event_type: &str, time_range: &str| -> i64 {
        debug!(event_type = %event_type, time_range = %time_range, "Count events (mock)");
        // TODO: 实际实现需要调用事件统计服务
        0
    });
    
    // record_event(event_type, data)
    engine.register_fn("record_event", |event_type: &str, data: rhai::Map| {
        info!(event_type = %event_type, "Record event (mock)");
        debug!("Event data: {:?}", data);
        // TODO: 实际实现需要调用事件记录服务
    });
}

/// 注册时间函数
fn register_time_functions(engine: &mut rhai::Engine) {
    use chrono::{Datelike, Timelike};
    
    // now() - 返回当前时间戳
    engine.register_fn("now", || -> rhai::Map {
        let now = chrono::Utc::now();
        let mut map = rhai::Map::new();
        map.insert("timestamp".into(), rhai::Dynamic::from(now.timestamp()));
        map.insert("hour".into(), rhai::Dynamic::from(now.hour() as i64));
        map.insert("minute".into(), rhai::Dynamic::from(now.minute() as i64));
        map.insert("month".into(), rhai::Dynamic::from(now.month() as i64));
        map.insert("day".into(), rhai::Dynamic::from(now.day() as i64));
        map.insert("weekday".into(), rhai::Dynamic::from(now.weekday().num_days_from_monday() as i64));
        map
    });
    
    // date_add(date, amount, unit)
    engine.register_fn("date_add", |_date: rhai::Map, _amount: i64, _unit: &str| -> rhai::Map {
        // TODO: 实现日期加减
        rhai::Map::new()
    });
    
    // format_date(date, format)
    engine.register_fn("format_date", |_date: rhai::Map, _format: &str| -> String {
        // TODO: 实现日期格式化
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
    });
    
    // date_start_of_day(date)
    engine.register_fn("date_start_of_day", |_date: rhai::Map| -> rhai::Map {
        // TODO: 实现获取日期开始
        rhai::Map::new()
    });
    
    // date_end_of_day(date)
    engine.register_fn("date_end_of_day", |_date: rhai::Map| -> rhai::Map {
        // TODO: 实现获取日期结束
        rhai::Map::new()
    });
}

/// 注册日志函数
fn register_log_functions(engine: &mut rhai::Engine) {
    // log(level, message)
    engine.register_fn("log", |level: &str, message: &str| {
        match level {
            "debug" => debug!("{}", message),
            "info" => info!("{}", message),
            "warn" => warn!("{}", message),
            "error" => error!("{}", message),
            _ => info!("{}", message),
        }
    });
    
    // debug(message)
    engine.register_fn("debug", |message: &str| {
        debug!("{}", message);
    });
    
    // info(message)
    engine.register_fn("info", |message: &str| {
        info!("{}", message);
    });
    
    // warn(message)
    engine.register_fn("warn", |message: &str| {
        warn!("{}", message);
    });
    
    // error(message)
    engine.register_fn("error", |message: &str| {
        error!("{}", message);
    });
}

/// 注册工单函数
pub fn register_ticket_functions(engine: &mut rhai::Engine) {
    // create_ticket(params)
    engine.register_fn("create_ticket", |params: rhai::Map| {
        info!("Create ticket (mock): {:?}", params);
        // TODO: 实际实现需要调用工单服务
    });
    
    // update_ticket(ticket_id, params)
    engine.register_fn("update_ticket", |ticket_id: &str, params: rhai::Map| {
        info!(ticket_id = %ticket_id, "Update ticket (mock)");
        debug!("Params: {:?}", params);
        // TODO: 实际实现需要调用工单服务
    });
    
    // close_ticket(ticket_id)
    engine.register_fn("close_ticket", |ticket_id: &str| {
        info!(ticket_id = %ticket_id, "Close ticket (mock)");
        // TODO: 实际实现需要调用工单服务
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_functions() {
        let mut engine = ScriptEngine::new();
        register_builtin_functions(&mut engine);
        
        // 测试函数是否注册成功
        let script = r#"
            log("info", "Test log");
            let time = now();
            control_device("test_device", "turn_on", #{});
        "#;
        
        assert!(engine.eval(script).is_ok());
    }
}
