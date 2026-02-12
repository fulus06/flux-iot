// RTSP 协议支持
use crate::engine::VideoStream;
use crate::Result;
use bytes::Bytes;
use retina::client::{Session, SessionOptions, SetupOptions};
use retina::codec::CodecItem;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use url::Url;
use futures::StreamExt;

/// RTSP 流状态
#[derive(Debug, Clone, PartialEq)]
pub enum StreamState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Stopped,
}

/// RTSP 流
pub struct RtspStream {
    stream_id: String,
    url: String,
    state: Arc<RwLock<StreamState>>,
    
    /// 媒体数据接收器
    media_rx: Option<mpsc::Receiver<MediaPacket>>,
    
    /// 重连配置
    reconnect_enabled: bool,
    max_reconnect_attempts: u32,
}

impl RtspStream {
    pub fn new(stream_id: String, url: String) -> Self {
        Self {
            stream_id,
            url,
            state: Arc::new(RwLock::new(StreamState::Disconnected)),
            media_rx: None,
            reconnect_enabled: true,
            max_reconnect_attempts: 10,
        }
    }
    
    /// 启动流
    pub async fn start(&mut self) -> Result<()> {
        *self.state.write().await = StreamState::Connecting;
        
        let url = Url::parse(&self.url)
            .map_err(|e| crate::VideoError::Other(format!("Invalid URL: {}", e)))?;
        
        // 创建媒体数据通道
        let (media_tx, media_rx) = mpsc::channel(100);
        self.media_rx = Some(media_rx);
        
        let stream_id = self.stream_id.clone();
        let state = self.state.clone();
        let reconnect_enabled = self.reconnect_enabled;
        let max_attempts = self.max_reconnect_attempts;
        
        // 启动接收任务
        tokio::spawn(async move {
            Self::receive_loop(
                stream_id,
                url,
                media_tx,
                state,
                reconnect_enabled,
                max_attempts,
            )
            .await;
        });
        
        tracing::info!("RTSP stream started: {}", self.stream_id);
        Ok(())
    }
    
    /// 停止流
    pub async fn stop(&mut self) -> Result<()> {
        *self.state.write().await = StreamState::Stopped;
        self.media_rx = None;
        
        tracing::info!("RTSP stream stopped: {}", self.stream_id);
        Ok(())
    }
    
    /// 接收媒体包
    pub async fn recv(&mut self) -> Option<MediaPacket> {
        if let Some(rx) = &mut self.media_rx {
            rx.recv().await
        } else {
            None
        }
    }
    
    /// 获取流状态
    pub async fn state(&self) -> StreamState {
        self.state.read().await.clone()
    }
    
    /// 接收循环（支持自动重连）
    async fn receive_loop(
        stream_id: String,
        url: Url,
        media_tx: mpsc::Sender<MediaPacket>,
        state: Arc<RwLock<StreamState>>,
        reconnect_enabled: bool,
        max_attempts: u32,
    ) {
        let mut attempt = 0;
        
        loop {
            // 检查是否应该停止
            if *state.read().await == StreamState::Stopped {
                break;
            }
            
            // 尝试连接
            match Self::connect_and_receive(&stream_id, &url, &media_tx, &state).await {
                Ok(_) => {
                    tracing::info!("RTSP stream {} ended normally", stream_id);
                    break;
                }
                Err(e) => {
                    tracing::error!("RTSP stream {} error: {}", stream_id, e);
                    
                    if !reconnect_enabled {
                        *state.write().await = StreamState::Disconnected;
                        break;
                    }
                    
                    attempt += 1;
                    if attempt >= max_attempts {
                        tracing::error!(
                            "RTSP stream {} exceeded max reconnect attempts ({})",
                            stream_id,
                            max_attempts
                        );
                        *state.write().await = StreamState::Disconnected;
                        break;
                    }
                    
                    // 指数退避重连
                    let delay = std::cmp::min(2u64.pow(attempt), 60);
                    tracing::info!(
                        "RTSP stream {} reconnecting in {} seconds (attempt {}/{})",
                        stream_id,
                        delay,
                        attempt,
                        max_attempts
                    );
                    
                    *state.write().await = StreamState::Reconnecting;
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                }
            }
        }
    }
    
    /// 连接并接收数据
    async fn connect_and_receive(
        stream_id: &str,
        url: &Url,
        media_tx: &mpsc::Sender<MediaPacket>,
        state: &Arc<RwLock<StreamState>>,
    ) -> Result<()> {
        tracing::info!("Connecting to RTSP stream: {}", url);
        
        // 创建 RTSP 会话
        let creds = if !url.username().is_empty() {
            Some(retina::client::Credentials {
                username: url.username().to_string(),
                password: url.password().unwrap_or("").to_string(),
            })
        } else {
            None
        };
        
        let session_options = SessionOptions::default()
            .creds(creds);
        
        let mut session = Session::describe(url.clone(), session_options)
            .await
            .map_err(|e| crate::VideoError::Other(format!("RTSP describe failed: {}", e)))?;
        
        // 设置视频流
        let video_stream_i = session
            .streams()
            .iter()
            .position(|s| matches!(s.media(), "video"))
            .ok_or_else(|| crate::VideoError::Other("No video stream found".to_string()))?;
        
        session
            .setup(video_stream_i, SetupOptions::default())
            .await
            .map_err(|e| crate::VideoError::Other(format!("RTSP setup failed: {}", e)))?;
        
        // 开始播放
        let session = session
            .play(retina::client::PlayOptions::default())
            .await
            .map_err(|e| crate::VideoError::Other(format!("RTSP play failed: {}", e)))?
            .demuxed()
            .map_err(|e| crate::VideoError::Other(format!("Demux failed: {}", e)))?;
        
        *state.write().await = StreamState::Connected;
        tracing::info!("RTSP stream connected: {}", stream_id);
        
        // 接收数据
        tokio::pin!(session);
        loop {
            match session.next().await {
                Some(Ok(pkt)) => {
                    // 转换为 MediaPacket
                    if let Some(media_pkt) = Self::convert_packet(pkt) {
                        if media_tx.send(media_pkt).await.is_err() {
                            tracing::warn!("Media channel closed for stream: {}", stream_id);
                            break;
                        }
                    }
                }
                Some(Err(e)) => {
                    return Err(crate::VideoError::Other(format!("Receive error: {}", e)));
                }
                None => {
                    tracing::info!("RTSP stream ended: {}", stream_id);
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// 转换 retina 包为 MediaPacket
    fn convert_packet(pkt: CodecItem) -> Option<MediaPacket> {
        match pkt {
            CodecItem::VideoFrame(frame) => {
                Some(MediaPacket {
                    data: Bytes::copy_from_slice(frame.data()),
                    timestamp: frame.timestamp(),
                    is_key: frame.is_random_access_point(),
                    media_type: MediaType::Video,
                })
            }
            CodecItem::AudioFrame(frame) => {
                Some(MediaPacket {
                    data: Bytes::copy_from_slice(frame.data()),
                    timestamp: frame.timestamp(),
                    is_key: false,
                    media_type: MediaType::Audio,
                })
            }
            _ => None,
        }
    }
}

impl VideoStream for RtspStream {
    fn stream_id(&self) -> &str {
        &self.stream_id
    }
}

/// 媒体包
#[derive(Debug, Clone)]
pub struct MediaPacket {
    pub data: Bytes,
    pub timestamp: retina::Timestamp,
    pub is_key: bool,
    pub media_type: MediaType,
}

/// 媒体类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Video,
    Audio,
}
