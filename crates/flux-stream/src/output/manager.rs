use crate::stream::{ClientType, OutputStream, Protocol, QualityLevel};
use anyhow::Result;
use flux_media_core::types::StreamId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// 输出管理器
/// 负责管理流的多种输出格式，智能选择最佳协议
pub struct OutputManager {
    outputs: Arc<RwLock<HashMap<StreamId, Vec<OutputEntry>>>>,
}

#[derive(Debug, Clone)]
struct OutputEntry {
    protocol: Protocol,
    url: String,
    quality: QualityLevel,
    active_clients: usize,
}

impl OutputManager {
    pub fn new() -> Self {
        Self {
            outputs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册输出流
    pub async fn register_output(
        &self,
        stream_id: StreamId,
        protocol: Protocol,
        url: String,
        quality: QualityLevel,
    ) -> Result<()> {
        let mut outputs = self.outputs.write().await;
        
        let entry = OutputEntry {
            protocol,
            url: url.clone(),
            quality,
            active_clients: 0,
        };

        outputs
            .entry(stream_id.clone())
            .or_insert_with(Vec::new)
            .push(entry);

        info!(
            stream_id = %stream_id,
            protocol = ?protocol,
            url = %url,
            "Output registered"
        );

        Ok(())
    }

    /// 获取输出流（智能选择）
    pub async fn get_output(
        &self,
        stream_id: &StreamId,
        client_type: ClientType,
        preferred_protocol: Option<Protocol>,
    ) -> Result<OutputStream> {
        let outputs = self.outputs.read().await;
        
        let entries = outputs
            .get(stream_id)
            .ok_or_else(|| anyhow::anyhow!("No outputs for stream: {}", stream_id))?;

        // 如果指定了协议，优先使用
        if let Some(protocol) = preferred_protocol {
            if let Some(entry) = entries.iter().find(|e| e.protocol == protocol) {
                return Ok(OutputStream {
                    stream_id: stream_id.clone(),
                    protocol: entry.protocol,
                    url: entry.url.clone(),
                    quality: entry.quality,
                });
            }
        }

        // 根据客户端类型智能选择协议
        let protocol = self.select_protocol_for_client(client_type, entries);
        
        let entry = entries
            .iter()
            .find(|e| e.protocol == protocol)
            .or_else(|| entries.first())
            .ok_or_else(|| anyhow::anyhow!("No suitable output found"))?;

        Ok(OutputStream {
            stream_id: stream_id.clone(),
            protocol: entry.protocol,
            url: entry.url.clone(),
            quality: entry.quality,
        })
    }

    /// 根据客户端类型选择最佳协议
    fn select_protocol_for_client(&self, client_type: ClientType, entries: &[OutputEntry]) -> Protocol {
        let available_protocols: Vec<Protocol> = entries.iter().map(|e| e.protocol).collect();

        match client_type {
            ClientType::WebBrowser => {
                // Web 浏览器优先选择 HLS
                if available_protocols.contains(&Protocol::HttpFlv) {
                    Protocol::HttpFlv
                } else if available_protocols.contains(&Protocol::WebRTC) {
                    Protocol::WebRTC
                } else {
                    available_protocols.first().copied().unwrap_or(Protocol::RTMP)
                }
            }
            ClientType::MobileApp => {
                // 移动端优先选择 HLS（兼容性好）
                if available_protocols.contains(&Protocol::RTMP) {
                    Protocol::RTMP
                } else if available_protocols.contains(&Protocol::HttpFlv) {
                    Protocol::HttpFlv
                } else {
                    available_protocols.first().copied().unwrap_or(Protocol::RTMP)
                }
            }
            ClientType::Desktop => {
                // 桌面端优先选择 RTMP（低延迟）
                if available_protocols.contains(&Protocol::RTMP) {
                    Protocol::RTMP
                } else if available_protocols.contains(&Protocol::RTSP) {
                    Protocol::RTSP
                } else {
                    available_protocols.first().copied().unwrap_or(Protocol::RTMP)
                }
            }
            ClientType::IoTDevice => {
                // IoT 设备优先选择 RTSP（资源占用低）
                if available_protocols.contains(&Protocol::RTSP) {
                    Protocol::RTSP
                } else if available_protocols.contains(&Protocol::RTMP) {
                    Protocol::RTMP
                } else {
                    available_protocols.first().copied().unwrap_or(Protocol::RTSP)
                }
            }
            ClientType::Unknown => {
                // 未知类型默认选择第一个可用协议
                available_protocols.first().copied().unwrap_or(Protocol::RTMP)
            }
        }
    }

    /// 增加客户端计数
    pub async fn increment_client_count(&self, stream_id: &StreamId, protocol: Protocol) -> Result<()> {
        let mut outputs = self.outputs.write().await;
        
        if let Some(entries) = outputs.get_mut(stream_id) {
            if let Some(entry) = entries.iter_mut().find(|e| e.protocol == protocol) {
                entry.active_clients += 1;
                info!(
                    stream_id = %stream_id,
                    protocol = ?protocol,
                    clients = entry.active_clients,
                    "Client connected"
                );
            }
        }

        Ok(())
    }

    /// 减少客户端计数
    pub async fn decrement_client_count(&self, stream_id: &StreamId, protocol: Protocol) -> Result<()> {
        let mut outputs = self.outputs.write().await;
        
        if let Some(entries) = outputs.get_mut(stream_id) {
            if let Some(entry) = entries.iter_mut().find(|e| e.protocol == protocol) {
                if entry.active_clients > 0 {
                    entry.active_clients -= 1;
                }
                info!(
                    stream_id = %stream_id,
                    protocol = ?protocol,
                    clients = entry.active_clients,
                    "Client disconnected"
                );
            }
        }

        Ok(())
    }

    /// 列出所有输出
    pub async fn list_outputs(&self, stream_id: &StreamId) -> Vec<(Protocol, String, usize)> {
        let outputs = self.outputs.read().await;
        
        outputs
            .get(stream_id)
            .map(|entries| {
                entries
                    .iter()
                    .map(|e| (e.protocol, e.url.clone(), e.active_clients))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_get_output() {
        let manager = OutputManager::new();
        let stream_id = StreamId::new("test", "stream-001");

        manager
            .register_output(
                stream_id.clone(),
                Protocol::RTMP,
                "rtmp://localhost/live/test".to_string(),
                QualityLevel::High,
            )
            .await
            .unwrap();

        manager
            .register_output(
                stream_id.clone(),
                Protocol::HttpFlv,
                "http://localhost/flv/test.flv".to_string(),
                QualityLevel::High,
            )
            .await
            .unwrap();

        let output = manager
            .get_output(&stream_id, ClientType::WebBrowser, None)
            .await
            .unwrap();

        assert_eq!(output.protocol, Protocol::HttpFlv);
    }

    #[tokio::test]
    async fn test_client_count() {
        let manager = OutputManager::new();
        let stream_id = StreamId::new("test", "stream-001");

        manager
            .register_output(
                stream_id.clone(),
                Protocol::RTMP,
                "rtmp://localhost/live/test".to_string(),
                QualityLevel::High,
            )
            .await
            .unwrap();

        manager
            .increment_client_count(&stream_id, Protocol::RTMP)
            .await
            .unwrap();

        let outputs = manager.list_outputs(&stream_id).await;
        assert_eq!(outputs[0].2, 1);

        manager
            .decrement_client_count(&stream_id, Protocol::RTMP)
            .await
            .unwrap();

        let outputs = manager.list_outputs(&stream_id).await;
        assert_eq!(outputs[0].2, 0);
    }

    #[test]
    fn test_protocol_selection() {
        let manager = OutputManager::new();
        
        let entries = vec![
            OutputEntry {
                protocol: Protocol::RTMP,
                url: "rtmp://test".to_string(),
                quality: QualityLevel::High,
                active_clients: 0,
            },
            OutputEntry {
                protocol: Protocol::HttpFlv,
                url: "http://test".to_string(),
                quality: QualityLevel::High,
                active_clients: 0,
            },
        ];

        assert_eq!(
            manager.select_protocol_for_client(ClientType::WebBrowser, &entries),
            Protocol::HttpFlv
        );

        assert_eq!(
            manager.select_protocol_for_client(ClientType::Desktop, &entries),
            Protocol::RTMP
        );
    }
}
