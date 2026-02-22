use crate::context::StreamContext;
use crate::stream::ClientInfo;
use anyhow::Result;
use flux_config::TranscodeTrigger;
use tracing::debug;

/// 转码触发检测器
pub struct TriggerDetector;

impl TriggerDetector {
    pub fn new() -> Self {
        Self
    }

    /// 评估是否需要转码
    pub async fn evaluate(
        &self,
        context: &StreamContext,
        client_info: &ClientInfo,
        triggers: &[TranscodeTrigger],
    ) -> Result<bool> {
        for trigger in triggers {
            if self.check_trigger(context, client_info, trigger).await? {
                debug!(
                    stream_id = %context.stream_id,
                    trigger = ?trigger,
                    "Transcode trigger activated"
                );
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn check_trigger(
        &self,
        context: &StreamContext,
        client_info: &ClientInfo,
        trigger: &TranscodeTrigger,
    ) -> Result<bool> {
        match trigger {
            TranscodeTrigger::ProtocolSwitch => {
                let mut active_protocols = context.get_active_protocols().await;
                let requested_protocol = client_info.preferred_protocol;
                
                // 添加当前请求的协议
                active_protocols.insert(requested_protocol);
                
                // 如果有多于一种协议，触发转码
                Ok(active_protocols.len() > 1)
            }

            TranscodeTrigger::ClientThreshold { count } => {
                let client_count = context.get_client_count().await;
                Ok(client_count >= *count)
            }

            TranscodeTrigger::ClientVariety => {
                let client_types = context.get_client_types().await;
                Ok(client_types.len() > 1)
            }

            TranscodeTrigger::NetworkVariance { threshold } => {
                if let Some((min_bw, max_bw)) = context.get_bandwidth_range().await {
                    if max_bw == 0 {
                        return Ok(false);
                    }
                    let variance = (max_bw - min_bw) as f64 / max_bw as f64;
                    Ok(variance > *threshold)
                } else {
                    Ok(false)
                }
            }

            TranscodeTrigger::Never => Ok(false),
        }
    }
}

impl Default for TriggerDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ClientType, Protocol};
    use flux_config::StreamMode;
    use flux_media_core::types::StreamId;

    #[tokio::test]
    async fn test_protocol_switch_trigger() {
        let detector = TriggerDetector::new();
        let context = StreamContext::new(
            StreamId::new("test", "stream-001"),
            Protocol::RTSP,
            StreamMode::Auto { triggers: vec![] },
        );

        let client1 = ClientInfo {
            client_id: "client-1".to_string(),
            client_type: ClientType::WebBrowser,
            preferred_protocol: Protocol::RTMP,
            bandwidth_estimate: Some(2000),
            user_agent: None,
        };
        context.add_client(client1.clone()).await;

        let client2 = ClientInfo {
            client_id: "client-2".to_string(),
            client_type: ClientType::WebBrowser,
            preferred_protocol: Protocol::HttpFlv,
            bandwidth_estimate: Some(2000),
            user_agent: None,
        };

        let should_transcode = detector
            .check_trigger(&context, &client2, &TranscodeTrigger::ProtocolSwitch)
            .await
            .unwrap();

        assert!(should_transcode);
    }

    #[tokio::test]
    async fn test_client_threshold_trigger() {
        let detector = TriggerDetector::new();
        let context = StreamContext::new(
            StreamId::new("test", "stream-001"),
            Protocol::RTSP,
            StreamMode::Auto { triggers: vec![] },
        );

        for i in 0..5 {
            let client = ClientInfo {
                client_id: format!("client-{}", i),
                client_type: ClientType::WebBrowser,
                preferred_protocol: Protocol::RTMP,
                bandwidth_estimate: Some(2000),
                user_agent: None,
            };
            context.add_client(client).await;
        }

        let client = ClientInfo {
            client_id: "test".to_string(),
            client_type: ClientType::WebBrowser,
            preferred_protocol: Protocol::RTMP,
            bandwidth_estimate: Some(2000),
            user_agent: None,
        };

        let should_transcode = detector
            .check_trigger(&context, &client, &TranscodeTrigger::ClientThreshold { count: 5 })
            .await
            .unwrap();

        assert!(should_transcode);
    }
}
