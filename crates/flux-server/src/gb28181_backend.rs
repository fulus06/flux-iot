use anyhow::{anyhow, Result};
use async_trait::async_trait;
use flux_video::gb28181::sip::{Device, DeviceStatus, SipServer};
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;

#[async_trait]
pub trait Gb28181Backend: Send + Sync {
    async fn invite(&self, device_id: &str, channel_id: &str, rtp_port: u16) -> Result<String>;
    async fn bye(&self, call_id: &str) -> Result<()>;

    async fn query_catalog(&self, device_id: &str) -> Result<()>;
    async fn query_device_info(&self, device_id: &str) -> Result<()>;
    async fn query_device_status(&self, device_id: &str) -> Result<()>;

    async fn list_devices(&self) -> Result<Vec<Value>>;
    async fn get_device(&self, device_id: &str) -> Result<Option<Value>>;
    async fn list_device_channels(&self, device_id: &str) -> Result<Option<Vec<Value>>>;

    async fn snapshot(&self, stream_id: &str) -> Result<Option<Vec<u8>>>;
}

pub type Gb28181BackendRef = Arc<dyn Gb28181Backend>;

pub struct EmbeddedBackend {
    sip: Arc<SipServer>,
}

impl EmbeddedBackend {
    pub fn new(sip: Arc<SipServer>) -> Self {
        Self { sip }
    }

    fn device_to_json(device: &Device) -> Value {
        let status = match device.status {
            DeviceStatus::Online => "online",
            DeviceStatus::Offline => "offline",
            DeviceStatus::Registering => "registering",
        };

        serde_json::json!({
            "device_id": device.device_id,
            "name": device.name,
            "ip": device.ip,
            "port": device.port,
            "status": status,
            "register_time_ms": device.register_time.timestamp_millis(),
            "last_keepalive_ms": device.last_keepalive.timestamp_millis(),
            "expires": device.expires,
            "transport": device.transport,
            "manufacturer": device.manufacturer,
            "model": device.model,
            "firmware": device.firmware,
        })
    }

    fn channel_to_json(channel: &flux_video::gb28181::sip::Channel) -> Value {
        serde_json::json!({
            "channel_id": channel.channel_id,
            "name": channel.name,
            "manufacturer": channel.manufacturer,
            "model": channel.model,
            "status": channel.status,
            "parent_id": channel.parent_id,
            "longitude": channel.longitude,
            "latitude": channel.latitude,
        })
    }
}

#[async_trait]
impl Gb28181Backend for EmbeddedBackend {
    async fn invite(&self, device_id: &str, channel_id: &str, rtp_port: u16) -> Result<String> {
        let call_id = self
            .sip
            .start_realtime_play(device_id, channel_id, rtp_port)
            .await?;
        Ok(call_id)
    }

    async fn bye(&self, call_id: &str) -> Result<()> {
        self.sip.stop_realtime_play(call_id).await?;
        Ok(())
    }

    async fn query_catalog(&self, device_id: &str) -> Result<()> {
        self.sip.query_catalog(device_id).await?;
        Ok(())
    }

    async fn query_device_info(&self, device_id: &str) -> Result<()> {
        self.sip.query_device_info(device_id).await?;
        Ok(())
    }

    async fn query_device_status(&self, device_id: &str) -> Result<()> {
        self.sip.query_device_status(device_id).await?;
        Ok(())
    }

    async fn list_devices(&self) -> Result<Vec<Value>> {
        let devices = self.sip.device_manager().list_devices().await;
        Ok(devices.iter().map(Self::device_to_json).collect())
    }

    async fn get_device(&self, device_id: &str) -> Result<Option<Value>> {
        let device_opt = self.sip.device_manager().get_device(device_id).await;
        Ok(device_opt.as_ref().map(Self::device_to_json))
    }

    async fn list_device_channels(&self, device_id: &str) -> Result<Option<Vec<Value>>> {
        let Some(device) = self.sip.device_manager().get_device(device_id).await else {
            return Ok(None);
        };
        Ok(Some(
            device
                .channels
                .iter()
                .map(Self::channel_to_json)
                .collect(),
        ))
    }

    async fn snapshot(&self, _stream_id: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }
}

pub struct RemoteBackend {
    base_url: String,
    client: Client,
}

impl RemoteBackend {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        format!("{}/{}", base, path)
    }

    fn is_not_found(err: &anyhow::Error) -> bool {
        let msg = err.to_string();
        msg.contains("404") || msg.to_ascii_lowercase().contains("not found")
    }

    async fn post_json(&self, path: &str, body: Value) -> Result<Value> {
        let url = self.url(path);
        let resp = self.client.post(url).json(&body).send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("remote gb28181 request failed: status={} body={}", status, text));
        }

        let v: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow!("remote gb28181 invalid json: {} (body={})", e, text))?;
        Ok(v)
    }

    async fn get_json(&self, path: &str) -> Result<Value> {
        let url = self.url(path);
        let resp = self.client.get(url).send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("remote gb28181 request failed: status={} body={}", status, text));
        }

        let v: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow!("remote gb28181 invalid json: {} (body={})", e, text))?;
        Ok(v)
    }
}

#[async_trait]
impl Gb28181Backend for RemoteBackend {
    async fn invite(&self, device_id: &str, channel_id: &str, rtp_port: u16) -> Result<String> {
        let v = self
            .post_json(
                "/api/v1/gb28181/invite",
                serde_json::json!({
                    "device_id": device_id,
                    "channel_id": channel_id,
                    "rtp_port": rtp_port,
                }),
            )
            .await?;

        v.get("call_id")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("remote invite missing call_id"))
    }

    async fn bye(&self, call_id: &str) -> Result<()> {
        let _ = self
            .post_json(
                "/api/v1/gb28181/bye",
                serde_json::json!({ "call_id": call_id }),
            )
            .await?;
        Ok(())
    }

    async fn query_catalog(&self, device_id: &str) -> Result<()> {
        let _ = self
            .post_json(
                "/api/v1/gb28181/catalog",
                serde_json::json!({ "device_id": device_id }),
            )
            .await?;
        Ok(())
    }

    async fn query_device_info(&self, device_id: &str) -> Result<()> {
        let _ = self
            .post_json(
                "/api/v1/gb28181/device-info",
                serde_json::json!({ "device_id": device_id }),
            )
            .await?;
        Ok(())
    }

    async fn query_device_status(&self, device_id: &str) -> Result<()> {
        let _ = self
            .post_json(
                "/api/v1/gb28181/device-status",
                serde_json::json!({ "device_id": device_id }),
            )
            .await?;
        Ok(())
    }

    async fn list_devices(&self) -> Result<Vec<Value>> {
        let v = self.get_json("/api/v1/gb28181/devices").await?;
        let Some(items) = v.get("devices").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        Ok(items.to_vec())
    }

    async fn get_device(&self, device_id: &str) -> Result<Option<Value>> {
        let v = self
            .get_json(&format!("/api/v1/gb28181/devices/{}", device_id))
            .await;

        match v {
            Ok(v) => Ok(v.get("device").cloned()),
            Err(e) => {
                if Self::is_not_found(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn list_device_channels(&self, device_id: &str) -> Result<Option<Vec<Value>>> {
        let v = self
            .get_json(&format!(
                "/api/v1/gb28181/devices/{}/channels",
                device_id
            ))
            .await;

        match v {
            Ok(v) => {
                let Some(items) = v.get("channels").and_then(Value::as_array) else {
                    return Ok(Some(Vec::new()));
                };
                Ok(Some(items.to_vec()))
            }
            Err(e) => {
                if Self::is_not_found(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn snapshot(&self, stream_id: &str) -> Result<Option<Vec<u8>>> {
        let encoded_stream_id = stream_id.replace("/", "%2F");
        let url = self.url(&format!("/api/v1/gb28181/streams/{}/snapshot", encoded_stream_id));
        let resp = self.client.get(url).send().await?;
        let status = resp.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "remote gb28181 snapshot request failed: status={} body={}",
                status,
                text
            ));
        }

        let bytes = resp.bytes().await?;
        Ok(Some(bytes.to_vec()))
    }
}
