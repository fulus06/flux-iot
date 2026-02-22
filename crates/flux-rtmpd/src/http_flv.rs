use anyhow::Result;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use flux_media_core::playback::flv::{FlvMuxer, FlvTag, FlvTagType};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tracing::{debug, error, info};

use crate::stream_manager::StreamManager;

/// HTTP-FLV 服务器
pub struct HttpFlvServer {
    stream_manager: Arc<StreamManager>,
}

impl HttpFlvServer {
    pub fn new(stream_manager: Arc<StreamManager>) -> Self {
        Self { stream_manager }
    }

    /// 处理 HTTP-FLV 流请求
    pub async fn handle_stream(
        &self,
        app_name: String,
        stream_key: String,
    ) -> Result<Response, StatusCode> {
        let stream_id = format!("{}/{}", app_name, stream_key);
        info!(target: "http_flv", stream = %stream_id, "HTTP-FLV stream requested");

        // 检查流是否存在
        if !self.stream_manager.stream_exists(&app_name, &stream_key).await {
            error!(target: "http_flv", stream = %stream_id, "Stream not found");
            return Err(StatusCode::NOT_FOUND);
        }

        // 订阅流
        let (video_rx, audio_rx) = match self
            .stream_manager
            .subscribe(&app_name, &stream_key)
            .await
        {
            Ok(rx) => rx,
            Err(e) => {
                error!(target: "http_flv", "Failed to subscribe: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        // 创建 FLV 流
        let stream = create_flv_stream(video_rx, audio_rx);

        // 返回 HTTP 响应
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "video/x-flv")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive")
            .body(Body::from_stream(stream))
            .unwrap())
    }
}

/// 创建 FLV 流
fn create_flv_stream(
    video_rx: broadcast::Receiver<crate::stream_manager::MediaPacket>,
    audio_rx: broadcast::Receiver<crate::stream_manager::MediaPacket>,
) -> impl futures::Stream<Item = Result<Bytes, std::io::Error>> {
    async_stream::stream! {
        let mut flv_muxer = FlvMuxer::new();
        
        // 发送 FLV 头部
        let header = flv_muxer.generate_header();
        yield Ok(header);

        // 创建视频和音频流
        let mut video_stream = BroadcastStream::new(video_rx);
        let mut audio_stream = BroadcastStream::new(audio_rx);

        loop {
            tokio::select! {
                // 处理视频数据
                Some(Ok(packet)) = video_stream.next() => {
                    let tag = FlvTag {
                        tag_type: FlvTagType::Video,
                        timestamp: packet.timestamp,
                        data: packet.data,
                    };

                    match flv_muxer.mux_tag(&tag) {
                        Ok(flv_data) => {
                            debug!(target: "http_flv", 
                                timestamp = packet.timestamp,
                                size = flv_data.len(),
                                "Video FLV tag sent"
                            );
                            yield Ok(flv_data);
                        }
                        Err(e) => {
                            error!(target: "http_flv", "Failed to mux video tag: {}", e);
                            break;
                        }
                    }
                }

                // 处理音频数据
                Some(Ok(packet)) = audio_stream.next() => {
                    let tag = FlvTag {
                        tag_type: FlvTagType::Audio,
                        timestamp: packet.timestamp,
                        data: packet.data,
                    };

                    match flv_muxer.mux_tag(&tag) {
                        Ok(flv_data) => {
                            debug!(target: "http_flv", 
                                timestamp = packet.timestamp,
                                size = flv_data.len(),
                                "Audio FLV tag sent"
                            );
                            yield Ok(flv_data);
                        }
                        Err(e) => {
                            error!(target: "http_flv", "Failed to mux audio tag: {}", e);
                            break;
                        }
                    }
                }

                // 流结束
                else => {
                    info!(target: "http_flv", "Stream ended");
                    break;
                }
            }
        }
    }
}

/// HTTP-FLV 路由处理器
pub async fn http_flv_handler(
    State(server): State<Arc<HttpFlvServer>>,
    Path((app_name, stream_key)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    server.handle_stream(app_name, stream_key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_flv_server_creation() {
        let stream_manager = Arc::new(StreamManager::new());
        let _server = HttpFlvServer::new(stream_manager);
    }
}
