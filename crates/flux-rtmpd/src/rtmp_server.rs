use crate::hls_manager::HlsManager;
use crate::media_processor::MediaProcessor;
use crate::stream_manager::StreamManager;
use anyhow::Result;
use bytes::Bytes;
use flux_media_core::types::StreamId;
use rml_rtmp::sessions::{
    ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

pub struct RtmpServer {
    bind_addr: String,
    sessions: Arc<RwLock<HashMap<usize, RtmpSession>>>,
    next_session_id: Arc<RwLock<usize>>,
    media_processor: Arc<MediaProcessor>,
    active_streams: Arc<RwLock<HashMap<String, ActiveStream>>>,
    stream_manager: Arc<StreamManager>,
    hls_manager: Arc<HlsManager>,
}

#[derive(Debug, Clone)]
pub struct ActiveStream {
    pub stream_id: StreamId,
    pub app_name: String,
    pub stream_key: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub video_frames: u64,
    pub audio_frames: u64,
}

struct RtmpSession {
    id: usize,
    session: ServerSession,
    app_name: Option<String>,
    stream_key: Option<String>,
}

impl RtmpServer {
    pub fn new(
        bind_addr: String,
        media_processor: Arc<MediaProcessor>,
        stream_manager: Arc<StreamManager>,
        hls_manager: Arc<HlsManager>,
    ) -> Self {
        Self {
            bind_addr,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            next_session_id: Arc::new(RwLock::new(1)),
            media_processor,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            stream_manager,
            hls_manager,
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!(target: "rtmpd", "RTMP server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    info!(target: "rtmpd", "New RTMP connection from {}", addr);
                    let server = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(socket).await {
                            error!(target: "rtmpd", "Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!(target: "rtmpd", "Accept error: {}", e);
                }
            }
        }
    }

    async fn handle_connection(&self, mut socket: TcpStream) -> Result<()> {
        let session_id = {
            let mut id = self.next_session_id.write().await;
            let current = *id;
            *id += 1;
            current
        };

        let config = ServerSessionConfig::new();
        let (session, initial_results) = ServerSession::new(config)?;

        let rtmp_session = RtmpSession {
            id: session_id,
            session,
            app_name: None,
            stream_key: None,
        };

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id, rtmp_session);
        }

        // 处理初始结果
        for result in initial_results {
            if let Err(e) = self.handle_session_result(session_id, result, &mut socket).await {
                error!(target: "rtmpd", session_id = session_id, "Initial result error: {}", e);
            }
        }

        // 主循环：读取数据并处理
        let mut buffer = vec![0u8; 4096];
        loop {
            match socket.read(&mut buffer).await {
                Ok(0) => {
                    info!(target: "rtmpd", session_id = session_id, "Connection closed");
                    break;
                }
                Ok(n) => {
                    let data = &buffer[..n];
                    if let Err(e) = self.process_data(session_id, data, &mut socket).await {
                        error!(target: "rtmpd", session_id = session_id, "Process error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!(target: "rtmpd", session_id = session_id, "Read error: {}", e);
                    break;
                }
            }
        }

        // 清理会话
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(&session_id);
        }

        Ok(())
    }

    async fn process_data(
        &self,
        session_id: usize,
        data: &[u8],
        socket: &mut TcpStream,
    ) -> Result<()> {
        let results = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

            session.session.handle_input(data)?
        };

        for result in results {
            self.handle_session_result(session_id, result, socket)
                .await?;
        }

        Ok(())
    }

    async fn handle_session_result(
        &self,
        session_id: usize,
        result: ServerSessionResult,
        socket: &mut TcpStream,
    ) -> Result<()> {
        match result {
            ServerSessionResult::OutboundResponse(packet) => {
                socket.write_all(&packet.bytes).await?;
            }
            ServerSessionResult::RaisedEvent(event) => {
                self.handle_event(session_id, event).await?;
            }
            ServerSessionResult::UnhandleableMessageReceived(_) => {
                warn!(target: "rtmpd", session_id = session_id, "Unhandleable message");
            }
        }
        Ok(())
    }

    async fn handle_event(&self, session_id: usize, event: ServerSessionEvent) -> Result<()> {
        match event {
            ServerSessionEvent::ConnectionRequested {
                request_id,
                app_name,
            } => {
                info!(target: "rtmpd", session_id = session_id, app = %app_name, "Connection requested");

                let mut sessions = self.sessions.write().await;
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.app_name = Some(app_name.clone());
                    let results = session.session.accept_request(request_id)?;
                    // TODO: 处理 accept 结果
                    drop(results);
                }
            }
            ServerSessionEvent::PublishStreamRequested {
                request_id,
                app_name,
                stream_key,
                mode: _,
            } => {
                info!(target: "rtmpd", 
                    session_id = session_id, 
                    app = %app_name, 
                    key = %stream_key, 
                    "Publish requested"
                );

                // 注册活跃流
                let stream_id = StreamId::new("rtmp", &format!("{}/{}", app_name, stream_key));
                let stream_key_full = format!("{}/{}", app_name, stream_key);
                
                let mut active_streams = self.active_streams.write().await;
                active_streams.insert(stream_key_full.clone(), ActiveStream {
                    stream_id: stream_id.clone(),
                    app_name: app_name.clone(),
                    stream_key: stream_key.clone(),
                    start_time: chrono::Utc::now(),
                    video_frames: 0,
                    audio_frames: 0,
                });
                drop(active_streams);

                // 注册到流管理器（用于播放分发）
                if let Err(e) = self.stream_manager.register_stream(app_name.clone(), stream_key.clone()).await {
                    error!(target: "rtmpd", "Failed to register stream: {}", e);
                }

                // 注册到 HLS 管理器（用于 HLS 转换）
                if let Err(e) = self.hls_manager.register_stream(app_name.clone(), stream_key.clone(), 6).await {
                    error!(target: "rtmpd", "Failed to register HLS stream: {}", e);
                }

                let mut sessions = self.sessions.write().await;
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.app_name = Some(app_name.clone());
                    session.stream_key = Some(stream_key.clone());
                    let results = session.session.accept_request(request_id)?;
                    for result in results {
                        // 处理 accept 结果的响应
                        if let ServerSessionResult::OutboundResponse(_) = result {
                            // 响应已在主循环中处理
                        }
                    }
                }
            }
            ServerSessionEvent::StreamMetadataChanged {
                app_name,
                stream_key,
                metadata,
            } => {
                info!(target: "rtmpd", 
                    app = %app_name, 
                    key = %stream_key, 
                    "Metadata: {:?}", metadata
                );
            }
            ServerSessionEvent::VideoDataReceived {
                app_name,
                stream_key,
                data,
                timestamp,
            } => {
                let stream_id = StreamId::new("rtmp", &format!("{}/{}", app_name, stream_key));
                let stream_key_full = format!("{}/{}", app_name, stream_key);
                
                // 更新视频帧计数
                if let Some(stream) = self.active_streams.write().await.get_mut(&stream_key_full) {
                    stream.video_frames += 1;
                }
                
                // 发布到流管理器（用于播放分发）
                let is_keyframe = !data.is_empty() && (data[0] >> 4) == 1;
                if let Err(e) = self.stream_manager.publish_video(
                    &app_name,
                    &stream_key,
                    Bytes::copy_from_slice(&data),
                    timestamp.value,
                    is_keyframe,
                ).await {
                    error!(target: "rtmpd", "Failed to publish video: {}", e);
                }

                // 发送到 HLS 管理器（用于 TS 分片）
                if let Err(e) = self.hls_manager.process_video(
                    &app_name,
                    &stream_key,
                    &data,
                    timestamp.value,
                    is_keyframe,
                ).await {
                    error!(target: "rtmpd", "Failed to process HLS video: {}", e);
                }
                
                if let Err(e) = self
                    .media_processor
                    .process_video(&stream_id, &data, timestamp.value)
                    .await
                {
                    error!(target: "rtmpd", 
                        app = %app_name, 
                        key = %stream_key, 
                        "Video processing error: {}", e
                    );
                }
            }
            ServerSessionEvent::AudioDataReceived {
                app_name,
                stream_key,
                data,
                timestamp,
            } => {
                let stream_id = StreamId::new("rtmp", &format!("{}/{}", app_name, stream_key));
                let stream_key_full = format!("{}/{}", app_name, stream_key);
                
                // 更新音频帧计数
                if let Some(stream) = self.active_streams.write().await.get_mut(&stream_key_full) {
                    stream.audio_frames += 1;
                }
                
                // 发布到流管理器（用于播放分发）
                if let Err(e) = self.stream_manager.publish_audio(
                    &app_name,
                    &stream_key,
                    Bytes::copy_from_slice(&data),
                    timestamp.value,
                ).await {
                    error!(target: "rtmpd", "Failed to publish audio: {}", e);
                }

                // 发送到 HLS 管理器（用于音频 TS 封装）
                if let Err(e) = self.hls_manager.process_audio(
                    &app_name,
                    &stream_key,
                    &data,
                    timestamp.value,
                ).await {
                    error!(target: "rtmpd", "Failed to process HLS audio: {}", e);
                }
                
                if let Err(e) = self
                    .media_processor
                    .process_audio(&stream_id, &data, timestamp.value)
                    .await
                {
                    error!(target: "rtmpd", 
                        app = %app_name, 
                        key = %stream_key, 
                        "Audio processing error: {}", e
                    );
                }
            }
            _ => {
                info!(target: "rtmpd", session_id = session_id, "Other event: {:?}", event);
            }
        }

        Ok(())
    }

    pub async fn get_active_streams(&self) -> Vec<ActiveStream> {
        let streams = self.active_streams.read().await;
        streams.values().cloned().collect()
    }
    
    pub async fn get_stream_info(&self, stream_key: &str) -> Option<ActiveStream> {
        let streams = self.active_streams.read().await;
        streams.get(stream_key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtmp_server_creation() {
        use crate::hls_manager::HlsManager;
        use crate::media_processor::MediaProcessor;
        use crate::stream_manager::StreamManager;
        use flux_media_core::storage::{filesystem::FileSystemStorage, StorageConfig};
        use flux_media_core::snapshot::SnapshotOrchestrator;
        use std::path::PathBuf;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            root_dir: temp_dir.path().to_path_buf(),
            retention_days: 7,
            segment_duration_secs: 60,
        };
        let storage = Arc::new(RwLock::new(FileSystemStorage::new(config).unwrap()));
        let orchestrator = Arc::new(SnapshotOrchestrator::new(temp_dir.path().to_path_buf()));
        let media_processor = Arc::new(MediaProcessor::new(storage, orchestrator));
        let stream_manager = Arc::new(StreamManager::new());
        let hls_dir = temp_dir.path().join("hls");
        let hls_manager = Arc::new(HlsManager::new(hls_dir));

        let server = RtmpServer::new("127.0.0.1:1935".to_string(), media_processor, stream_manager, hls_manager);
        assert_eq!(server.bind_addr, "127.0.0.1:1935");
    }

    #[tokio::test]
    async fn test_session_id_increment() {
        use crate::hls_manager::HlsManager;
        use crate::media_processor::MediaProcessor;
        use crate::stream_manager::StreamManager;
        use flux_media_core::storage::{filesystem::FileSystemStorage, StorageConfig};
        use flux_media_core::snapshot::SnapshotOrchestrator;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            root_dir: temp_dir.path().to_path_buf(),
            retention_days: 7,
            segment_duration_secs: 60,
        };
        let storage = Arc::new(RwLock::new(FileSystemStorage::new(config).unwrap()));
        let orchestrator = Arc::new(SnapshotOrchestrator::new(temp_dir.path().to_path_buf()));
        let media_processor = Arc::new(MediaProcessor::new(storage, orchestrator));
        let stream_manager = Arc::new(StreamManager::new());
        let hls_dir = temp_dir.path().join("hls");
        let hls_manager = Arc::new(HlsManager::new(hls_dir));

        let server = Arc::new(RtmpServer::new("127.0.0.1:1935".to_string(), media_processor, stream_manager, hls_manager));
        let id1 = *server.next_session_id.read().await;
        {
            let mut id = server.next_session_id.write().await;
            *id += 1;
        }
        let id2 = *server.next_session_id.read().await;
        assert_eq!(id2, id1 + 1);
    }
}
