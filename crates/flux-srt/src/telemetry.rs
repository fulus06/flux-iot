use serde_json::json;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone)]
pub struct TelemetryClient {
    endpoint: Option<String>,
    timeout: Duration,
    client: reqwest::Client,
}

impl TelemetryClient {
    pub fn new(endpoint: Option<String>, timeout_ms: u64) -> Self {
        Self {
            endpoint: endpoint.filter(|s| !s.is_empty()),
            timeout: Duration::from_millis(timeout_ms),
            client: reqwest::Client::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.endpoint.is_some()
    }

    pub async fn post(&self, topic: &str, payload: Value) {
        let Some(endpoint) = self.endpoint.as_ref() else {
            return;
        };

        let req_body = json!({
            "topic": topic,
            "payload": payload,
        });

        match self
            .client
            .post(endpoint)
            .timeout(self.timeout)
            .json(&req_body)
            .send()
            .await
        {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::warn!(
                        target: "srt",
                        "telemetry post failed: status={} topic={} endpoint={}",
                        resp.status(),
                        topic,
                        endpoint
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "srt",
                    "telemetry post error: {} topic={} endpoint={}",
                    e,
                    topic,
                    endpoint
                );
            }
        }
    }
}
