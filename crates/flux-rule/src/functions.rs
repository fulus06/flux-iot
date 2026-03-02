use flux_script::ScriptEngine;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::services::{NoopRuleServices, RuleServices};

/// 注册所有内置函数
pub fn register_builtin_functions(engine: &mut ScriptEngine) {
    register_builtin_functions_with_services(engine, Arc::new(NoopRuleServices));
}

/// 注册所有内置函数（带服务实现）
pub fn register_builtin_functions_with_services(
    engine: &mut ScriptEngine,
    services: Arc<dyn RuleServices>,
) {
    let rhai_engine = engine.engine_mut();

    register_device_functions(rhai_engine, services.clone());
    register_notification_functions(rhai_engine, services.clone());
    register_data_functions(rhai_engine, services.clone());
    register_time_functions(rhai_engine);
    register_log_functions(rhai_engine);
    register_ticket_functions(rhai_engine, services);
}

/// 注册设备控制函数
fn register_device_functions(engine: &mut rhai::Engine, services: Arc<dyn RuleServices>) {
    // control_device(device_id, command, params)
    let services_for_control = services.clone();
    engine.register_fn("control_device", move |device_id: &str, command: &str, params: rhai::Map| {
        let services = services_for_control.clone();
        let device_id = device_id.to_string();
        let command = command.to_string();

        let params_json = match serde_json::to_value(&params) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Failed to serialize params");
                serde_json::Value::Null
            }
        };

        info!(
            device_id = %device_id,
            command = %command,
            "Control device"
        );

        tokio::spawn(async move {
            if let Err(e) = services.control_device(&device_id, &command, params_json).await {
                warn!(device_id = %device_id, command = %command, error = %e, "control_device failed");
            }
        });
    });
    
    // read_device(device_id, metric)
    let services_for_read = services.clone();
    engine.register_fn("read_device", move |device_id: &str, metric: &str| -> rhai::Dynamic {
        debug!(device_id = %device_id, metric = %metric, "Read device");

        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            warn!("read_device called outside tokio runtime");
            return rhai::Dynamic::UNIT;
        };

        let device_id = device_id.to_string();
        let metric = metric.to_string();
        let services = services_for_read.clone();

        let result = tokio::task::block_in_place(|| {
            handle.block_on(async move { services.read_device(&device_id, &metric).await })
        });

        match result {
            Ok(v) => match rhai::serde::to_dynamic(&v) {
                Ok(d) => d,
                Err(e) => {
                    warn!(error = %e, "Failed to convert read_device result to rhai dynamic");
                    rhai::Dynamic::UNIT
                }
            },
            Err(e) => {
                warn!(error = %e, "read_device failed");
                rhai::Dynamic::UNIT
            }
        }
    });
    
    // update_device_status(device_id, status)
    let services_for_status = services;
    engine.register_fn("update_device_status", move |device_id: &str, status: &str| {
        let services = services_for_status.clone();
        let device_id = device_id.to_string();
        let status = status.to_string();

        info!(device_id = %device_id, status = %status, "Update device status (mock)");

        tokio::spawn(async move {
            if let Err(e) = services.update_device_status(&device_id, &status).await {
                warn!(device_id = %device_id, status = %status, error = %e, "update_device_status failed");
            }
        });
    });
}

/// 注册通知函数
fn register_notification_functions(engine: &mut rhai::Engine, services: Arc<dyn RuleServices>) {
    // send_notification(channel, title, message)
    let services_for_notify = services.clone();
    engine.register_fn("send_notification", move |channel: &str, title: &str, message: &str| {
        let services = services_for_notify.clone();
        let channel = channel.to_string();
        let title = title.to_string();
        let message = message.to_string();

        info!(
            channel = %channel,
            title = %title,
            message = %message,
            "Send notification"
        );

        tokio::spawn(async move {
            if let Err(e) = services.send_notification(&channel, &title, &message).await {
                warn!(channel = %channel, error = %e, "send_notification failed");
            }
        });
    });
    
    // send_email(to, subject, body)
    let services_for_email = services.clone();
    engine.register_fn("send_email", move |params: rhai::Map| {
        let services = services_for_email.clone();
        let params_json = match serde_json::to_value(&params) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Failed to serialize email params");
                serde_json::Value::Null
            }
        };

        info!("Send email");
        tokio::spawn(async move {
            if let Err(e) = services.send_email(params_json).await {
                warn!(error = %e, "send_email failed");
            }
        });
    });
    
    // send_sms(phone, message)
    let services_for_sms = services.clone();
    engine.register_fn("send_sms", move |phone: &str, message: &str| {
        let services = services_for_sms.clone();
        let phone = phone.to_string();
        let message = message.to_string();

        info!(phone = %phone, message = %message, "Send SMS");
        tokio::spawn(async move {
            if let Err(e) = services.send_sms(&phone, &message).await {
                warn!(phone = %phone, error = %e, "send_sms failed");
            }
        });
    });
    
    // send_push(user_id, title, message)
    let services_for_push = services;
    engine.register_fn("send_push", move |user_id: &str, title: &str, message: &str| {
        let services = services_for_push.clone();
        let user_id = user_id.to_string();
        let title = title.to_string();
        let message = message.to_string();

        info!(
            user_id = %user_id,
            title = %title,
            message = %message,
            "Send push notification"
        );

        tokio::spawn(async move {
            if let Err(e) = services.send_push(&user_id, &title, &message).await {
                warn!(user_id = %user_id, error = %e, "send_push failed");
            }
        });
    });
}

/// 注册数据查询函数
fn register_data_functions(engine: &mut rhai::Engine, services: Arc<dyn RuleServices>) {
    // query_metrics(params)
    let services_for_query = services.clone();
    engine.register_fn("query_metrics", move |params: rhai::Map| -> rhai::Map {
        debug!("Query metrics");

        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            warn!("query_metrics called outside tokio runtime");
            return rhai::Map::new();
        };

        let params_json = match serde_json::to_value(&params) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Failed to serialize query_metrics params");
                serde_json::Value::Null
            }
        };

        let services = services_for_query.clone();

        let result = tokio::task::block_in_place(|| {
            handle.block_on(async move { services.query_metrics(params_json).await })
        });

        let Ok(json) = result else {
            warn!("query_metrics failed");
            return rhai::Map::new();
        };

        let Ok(dyn_val) = rhai::serde::to_dynamic(&json) else {
            return rhai::Map::new();
        };

        dyn_val.try_cast::<rhai::Map>().unwrap_or_default()
    });

    // count_events(event_type, time_range)
    let services_for_count = services.clone();
    engine.register_fn("count_events", move |event_type: &str, time_range: &str| -> i64 {
        debug!(event_type = %event_type, time_range = %time_range, "Count events");

        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            warn!("count_events called outside tokio runtime");
            return 0;
        };

        let event_type = event_type.to_string();
        let time_range = time_range.to_string();
        let services = services_for_count.clone();

        let result = tokio::task::block_in_place(|| {
            handle.block_on(async move { services.count_events(&event_type, &time_range).await })
        });

        match result {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "count_events failed");
                0
            }
        }
    });

    // record_event(event_type, data)
    engine.register_fn("record_event", move |event_type: &str, data: rhai::Map| {
        let services = services.clone();
        let event_type = event_type.to_string();

        let data_json = match serde_json::to_value(&data) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Failed to serialize record_event data");
                serde_json::Value::Null
            }
        };

        info!(event_type = %event_type, "Record event");
        tokio::spawn(async move {
            if let Err(e) = services.record_event(&event_type, data_json).await {
                warn!(event_type = %event_type, error = %e, "record_event failed");
            }
        });
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
    engine.register_fn("date_add", |date: rhai::Map, amount: i64, unit: &str| -> rhai::Map {
        use chrono::{DateTime, Duration, Utc};
        
        // 从 map 中提取 timestamp
        let timestamp = date.get("timestamp")
            .and_then(|v| v.as_int().ok())
            .unwrap_or_else(|| Utc::now().timestamp());
        
        let dt = DateTime::from_timestamp(timestamp, 0)
            .unwrap_or_else(|| Utc::now());
        
        // 根据单位添加时间
        let new_dt = match unit {
            "seconds" | "second" | "s" => dt + Duration::seconds(amount),
            "minutes" | "minute" | "m" => dt + Duration::minutes(amount),
            "hours" | "hour" | "h" => dt + Duration::hours(amount),
            "days" | "day" | "d" => dt + Duration::days(amount),
            "weeks" | "week" | "w" => dt + Duration::weeks(amount),
            _ => dt, // 未知单位，返回原值
        };
        
        // 构造返回的 map
        let mut map = rhai::Map::new();
        map.insert("timestamp".into(), rhai::Dynamic::from(new_dt.timestamp()));
        map.insert("hour".into(), rhai::Dynamic::from(new_dt.hour() as i64));
        map.insert("minute".into(), rhai::Dynamic::from(new_dt.minute() as i64));
        map.insert("month".into(), rhai::Dynamic::from(new_dt.month() as i64));
        map.insert("day".into(), rhai::Dynamic::from(new_dt.day() as i64));
        map.insert("weekday".into(), rhai::Dynamic::from(new_dt.weekday().num_days_from_monday() as i64));
        map
    });
    
    // format_date(date, format)
    engine.register_fn("format_date", |date: rhai::Map, format: &str| -> String {
        use chrono::{DateTime, Utc};
        
        // 从 map 中提取 timestamp
        let timestamp = date.get("timestamp")
            .and_then(|v| v.as_int().ok())
            .unwrap_or_else(|| Utc::now().timestamp());
        
        let dt = DateTime::from_timestamp(timestamp, 0)
            .unwrap_or_else(|| Utc::now());
        
        dt.format(format).to_string()
    });
    
    // date_start_of_day(date)
    engine.register_fn("date_start_of_day", |date: rhai::Map| -> rhai::Map {
        use chrono::{DateTime, Timelike, Utc};
        
        // 从 map 中提取 timestamp
        let timestamp = date.get("timestamp")
            .and_then(|v| v.as_int().ok())
            .unwrap_or_else(|| Utc::now().timestamp());
        
        let dt = DateTime::from_timestamp(timestamp, 0)
            .unwrap_or_else(|| Utc::now());
        
        // 设置为当天 00:00:00
        let start_of_day = dt
            .with_hour(0).unwrap()
            .with_minute(0).unwrap()
            .with_second(0).unwrap()
            .with_nanosecond(0).unwrap();
        
        let mut map = rhai::Map::new();
        map.insert("timestamp".into(), rhai::Dynamic::from(start_of_day.timestamp()));
        map.insert("hour".into(), rhai::Dynamic::from(0i64));
        map.insert("minute".into(), rhai::Dynamic::from(0i64));
        map.insert("month".into(), rhai::Dynamic::from(start_of_day.month() as i64));
        map.insert("day".into(), rhai::Dynamic::from(start_of_day.day() as i64));
        map.insert("weekday".into(), rhai::Dynamic::from(start_of_day.weekday().num_days_from_monday() as i64));
        map
    });
    
    // date_end_of_day(date)
    engine.register_fn("date_end_of_day", |date: rhai::Map| -> rhai::Map {
        use chrono::{DateTime, Timelike, Utc};
        
        // 从 map 中提取 timestamp
        let timestamp = date.get("timestamp")
            .and_then(|v| v.as_int().ok())
            .unwrap_or_else(|| Utc::now().timestamp());
        
        let dt = DateTime::from_timestamp(timestamp, 0)
            .unwrap_or_else(|| Utc::now());
        
        // 设置为当天 23:59:59
        let end_of_day = dt
            .with_hour(23).unwrap()
            .with_minute(59).unwrap()
            .with_second(59).unwrap()
            .with_nanosecond(999_999_999).unwrap();
        
        let mut map = rhai::Map::new();
        map.insert("timestamp".into(), rhai::Dynamic::from(end_of_day.timestamp()));
        map.insert("hour".into(), rhai::Dynamic::from(23i64));
        map.insert("minute".into(), rhai::Dynamic::from(59i64));
        map.insert("month".into(), rhai::Dynamic::from(end_of_day.month() as i64));
        map.insert("day".into(), rhai::Dynamic::from(end_of_day.day() as i64));
        map.insert("weekday".into(), rhai::Dynamic::from(end_of_day.weekday().num_days_from_monday() as i64));
        map
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
pub fn register_ticket_functions(engine: &mut rhai::Engine, services: Arc<dyn RuleServices>) {
    // create_ticket(params)
    let services_for_create = services.clone();
    engine.register_fn("create_ticket", move |params: rhai::Map| {
        let services = services_for_create.clone();
        let params_json = match serde_json::to_value(&params) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Failed to serialize ticket params");
                serde_json::Value::Null
            }
        };

        info!("Create ticket");
        tokio::spawn(async move {
            if let Err(e) = services.create_ticket(params_json).await {
                warn!(error = %e, "create_ticket failed");
            }
        });
    });
    
    // update_ticket(ticket_id, params)
    let services_for_update = services.clone();
    engine.register_fn("update_ticket", move |ticket_id: &str, params: rhai::Map| {
        let services = services_for_update.clone();
        let ticket_id = ticket_id.to_string();
        let params_json = match serde_json::to_value(&params) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Failed to serialize ticket update params");
                serde_json::Value::Null
            }
        };

        info!(ticket_id = %ticket_id, "Update ticket");
        tokio::spawn(async move {
            if let Err(e) = services.update_ticket(&ticket_id, params_json).await {
                warn!(ticket_id = %ticket_id, error = %e, "update_ticket failed");
            }
        });
    });
    
    // close_ticket(ticket_id)
    let services_for_close = services;
    engine.register_fn("close_ticket", move |ticket_id: &str| {
        let services = services_for_close.clone();
        let ticket_id = ticket_id.to_string();

        info!(ticket_id = %ticket_id, "Close ticket");
        tokio::spawn(async move {
            if let Err(e) = services.close_ticket(&ticket_id).await {
                warn!(ticket_id = %ticket_id, error = %e, "close_ticket failed");
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::services::RuleServices;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestServices {
        control_calls: Arc<AtomicUsize>,
        notify_calls: Arc<AtomicUsize>,
        record_calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl RuleServices for TestServices {
        async fn control_device(
            &self,
            _device_id: &str,
            _command: &str,
            _params: serde_json::Value,
        ) -> anyhow::Result<()> {
            self.control_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        async fn read_device(&self, _device_id: &str, _metric: &str) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::Value::Null)
        }

        async fn update_device_status(&self, _device_id: &str, _status: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn send_notification(
            &self,
            _channel: &str,
            _title: &str,
            _message: &str,
        ) -> anyhow::Result<()> {
            self.notify_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        async fn send_email(&self, _params: serde_json::Value) -> anyhow::Result<()> {
            Ok(())
        }

        async fn send_sms(&self, _phone: &str, _message: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn send_push(&self, _user_id: &str, _title: &str, _message: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn query_metrics(&self, _params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({"total": 1, "average": 1, "peak": 1}))
        }

        async fn count_events(&self, _event_type: &str, _time_range: &str) -> anyhow::Result<i64> {
            Ok(0)
        }

        async fn record_event(&self, _event_type: &str, _data: serde_json::Value) -> anyhow::Result<()> {
            self.record_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        async fn create_ticket(&self, _params: serde_json::Value) -> anyhow::Result<()> {
            Ok(())
        }

        async fn update_ticket(&self, _ticket_id: &str, _params: serde_json::Value) -> anyhow::Result<()> {
            Ok(())
        }

        async fn close_ticket(&self, _ticket_id: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_register_functions() {
        let control_calls = Arc::new(AtomicUsize::new(0));
        let notify_calls = Arc::new(AtomicUsize::new(0));
        let record_calls = Arc::new(AtomicUsize::new(0));

        let services = Arc::new(TestServices {
            control_calls: control_calls.clone(),
            notify_calls: notify_calls.clone(),
            record_calls: record_calls.clone(),
        });

        let mut engine = ScriptEngine::new();
        register_builtin_functions_with_services(&mut engine, services);

        let script = r#"
            log("info", "Test log");
            let t = now();
            control_device("test_device", "turn_on", #{});
            send_notification("console", "t", "m");
            record_event("test", #{ value: 1 });
        "#;

        assert!(engine.eval(script).is_ok());

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(control_calls.load(Ordering::Relaxed) > 0);
        assert!(notify_calls.load(Ordering::Relaxed) > 0);
        assert!(record_calls.load(Ordering::Relaxed) > 0);
    }
}
