use crate::stream::{ClientInfo, ClientType, Protocol, StreamStatus};
use flux_config::{StreamMode, TranscodeTrigger};
use flux_media_core::types::StreamId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 流上下文（管理单个流的状态）
#[derive(Clone)]
pub struct StreamContext {
    pub stream_id: StreamId,
    pub input_protocol: Protocol,
    pub mode: StreamMode,
    pub status: StreamStatus,
    pub is_transcoding: bool,
    pub clients: Arc<RwLock<HashMap<String, ClientInfo>>>,
}

impl StreamContext {
    pub fn new(stream_id: StreamId, input_protocol: Protocol, mode: StreamMode) -> Self {
        Self {
            stream_id,
            input_protocol,
            mode,
            status: StreamStatus::Idle,
            is_transcoding: false,
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_client(&self, client: ClientInfo) {
        let mut clients = self.clients.write().await;
        clients.insert(client.client_id.clone(), client);
    }

    pub async fn remove_client(&self, client_id: &str) {
        let mut clients = self.clients.write().await;
        clients.remove(client_id);
    }

    pub async fn get_client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }

    pub async fn get_active_protocols(&self) -> HashSet<Protocol> {
        let clients = self.clients.read().await;
        clients
            .values()
            .map(|c| c.preferred_protocol)
            .collect()
    }

    pub async fn get_client_types(&self) -> HashSet<ClientType> {
        let clients = self.clients.read().await;
        clients
            .values()
            .map(|c| c.client_type)
            .collect()
    }

    pub async fn get_bandwidth_range(&self) -> Option<(u32, u32)> {
        let clients = self.clients.read().await;
        let bandwidths: Vec<u32> = clients
            .values()
            .filter_map(|c| c.bandwidth_estimate)
            .collect();

        if bandwidths.is_empty() {
            return None;
        }

        let min = *bandwidths.iter().min().unwrap();
        let max = *bandwidths.iter().max().unwrap();
        Some((min, max))
    }
}
