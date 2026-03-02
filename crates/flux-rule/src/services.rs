use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait RuleServices: Send + Sync {
    async fn control_device(
        &self,
        device_id: &str,
        command: &str,
        params: serde_json::Value,
    ) -> Result<()>;

    async fn read_device(&self, device_id: &str, metric: &str) -> Result<serde_json::Value>;

    async fn update_device_status(&self, device_id: &str, status: &str) -> Result<()>;

    async fn send_notification(&self, channel: &str, title: &str, message: &str) -> Result<()>;

    async fn send_email(&self, params: serde_json::Value) -> Result<()>;

    async fn send_sms(&self, phone: &str, message: &str) -> Result<()>;

    async fn send_push(&self, user_id: &str, title: &str, message: &str) -> Result<()>;

    async fn query_metrics(&self, params: serde_json::Value) -> Result<serde_json::Value>;

    async fn count_events(&self, event_type: &str, time_range: &str) -> Result<i64>;

    async fn record_event(&self, event_type: &str, data: serde_json::Value) -> Result<()>;

    async fn create_ticket(&self, params: serde_json::Value) -> Result<()>;

    async fn update_ticket(&self, ticket_id: &str, params: serde_json::Value) -> Result<()>;

    async fn close_ticket(&self, ticket_id: &str) -> Result<()>;
}

pub struct NoopRuleServices;

#[async_trait]
impl RuleServices for NoopRuleServices {
    async fn control_device(
        &self,
        _device_id: &str,
        _command: &str,
        _params: serde_json::Value,
    ) -> Result<()> {
        Ok(())
    }

    async fn read_device(&self, _device_id: &str, _metric: &str) -> Result<serde_json::Value> {
        Ok(serde_json::Value::Null)
    }

    async fn update_device_status(&self, _device_id: &str, _status: &str) -> Result<()> {
        Ok(())
    }

    async fn send_notification(&self, _channel: &str, _title: &str, _message: &str) -> Result<()> {
        Ok(())
    }

    async fn send_email(&self, _params: serde_json::Value) -> Result<()> {
        Ok(())
    }

    async fn send_sms(&self, _phone: &str, _message: &str) -> Result<()> {
        Ok(())
    }

    async fn send_push(&self, _user_id: &str, _title: &str, _message: &str) -> Result<()> {
        Ok(())
    }

    async fn query_metrics(&self, _params: serde_json::Value) -> Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn count_events(&self, _event_type: &str, _time_range: &str) -> Result<i64> {
        Ok(0)
    }

    async fn record_event(&self, _event_type: &str, _data: serde_json::Value) -> Result<()> {
        Ok(())
    }

    async fn create_ticket(&self, _params: serde_json::Value) -> Result<()> {
        Ok(())
    }

    async fn update_ticket(&self, _ticket_id: &str, _params: serde_json::Value) -> Result<()> {
        Ok(())
    }

    async fn close_ticket(&self, _ticket_id: &str) -> Result<()> {
        Ok(())
    }
}
