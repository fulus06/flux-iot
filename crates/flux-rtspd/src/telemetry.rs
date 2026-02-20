use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct TelemetryClient {
    endpoint: Option<String>,
    timeout: Duration,
    client: reqwest::Client,
    counters: Arc<Mutex<HashMap<String, u64>>>,
}

impl TelemetryClient {
    pub fn new(endpoint: Option<String>, timeout_ms: u64) -> Self {
        Self {
            endpoint: endpoint.filter(|s| !s.is_empty()),
            timeout: Duration::from_millis(timeout_ms),
            client: reqwest::Client::new(),
            counters: Arc::new(Mutex::new(HashMap::new())),
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
                        target: "rtspd",
                        "telemetry post failed: status={} topic={} endpoint={}",
                        resp.status(),
                        topic,
                        endpoint
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "rtspd",
                    "telemetry post error: {} topic={} endpoint={}",
                    e,
                    topic,
                    endpoint
                );
            }
        }
    }

    pub async fn post_sampled(&self, topic: &str, payload: Value, every_n: u64) {
        if every_n <= 1 {
            self.post(topic, payload).await;
            return;
        }

        let mut guard = self.counters.lock().await;
        let v = guard.entry(topic.to_string()).or_insert(0);
        *v = v.saturating_add(1);
        let should_send = (*v % every_n) == 0;
        drop(guard);

        if should_send {
            self.post(topic, payload).await;
        }
    }
}
