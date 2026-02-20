use anyhow::{anyhow, Result};
use bytes::Bytes;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// 传输模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    Udp,        // UDP 单播
    Tcp,        // TCP 单播
    Multicast,  // UDP 多播
}

/// Interleaved 数据包
#[derive(Debug, Clone)]
pub struct InterleavedPacket {
    pub channel: u8,
    pub data: Bytes,
}

/// RTSP 客户端
pub struct RtspClient {
    stream: Option<TcpStream>,
    url: String,
    session_id: Option<String>,
    cseq: u32,
    transport_mode: TransportMode,
}

/// RTSP 响应
#[derive(Debug)]
pub struct RtspResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl RtspClient {
    pub fn new(url: String) -> Self {
        Self {
            stream: None,
            url,
            session_id: None,
            cseq: 1,
            transport_mode: TransportMode::Udp,
        }
    }

    /// 设置传输模式
    pub fn set_transport_mode(&mut self, mode: TransportMode) {
        self.transport_mode = mode;
    }

    /// 连接到 RTSP 服务器
    pub async fn connect(&mut self) -> Result<()> {
        let url = self.parse_url()?;
        let addr = format!("{}:{}", url.host, url.port);
        
        info!(target: "rtsp_client", "Connecting to {}", addr);
        let stream = TcpStream::connect(&addr).await?;
        self.stream = Some(stream);
        
        Ok(())
    }

    /// 发送 OPTIONS 请求
    pub async fn options(&mut self) -> Result<RtspResponse> {
        let request = format!(
            "OPTIONS {} RTSP/1.0\r\nCSeq: {}\r\n\r\n",
            self.url, self.cseq
        );
        self.cseq += 1;
        
        self.send_request(&request).await
    }

    /// 发送 DESCRIBE 请求
    pub async fn describe(&mut self) -> Result<RtspResponse> {
        let request = format!(
            "DESCRIBE {} RTSP/1.0\r\nCSeq: {}\r\nAccept: application/sdp\r\n\r\n",
            self.url, self.cseq
        );
        self.cseq += 1;
        
        self.send_request(&request).await
    }

    /// 发送 SETUP 请求
    pub async fn setup(&mut self, track_url: &str, client_port: u16) -> Result<RtspResponse> {
        let transport = match self.transport_mode {
            TransportMode::Udp => {
                format!("RTP/AVP;unicast;client_port={}-{}", client_port, client_port + 1)
            }
            TransportMode::Tcp => {
                // TCP Interleaved 模式
                // channel 0: RTP, channel 1: RTCP
                format!("RTP/AVP/TCP;unicast;interleaved=0-1")
            }
            TransportMode::Multicast => {
                // 多播模式
                // 服务器会在响应中返回多播地址和端口
                format!("RTP/AVP;multicast")
            }
        };
        
        let request = format!(
            "SETUP {} RTSP/1.0\r\nCSeq: {}\r\nTransport: {}\r\n\r\n",
            track_url, self.cseq, transport
        );
        self.cseq += 1;
        
        let response = self.send_request(&request).await?;
        
        // 提取 Session ID
        if let Some(session) = response.get_header("Session") {
            self.session_id = Some(session.split(';').next().unwrap_or(&session).to_string());
            info!(target: "rtsp_client", "Session ID: {}", session);
        }
        
        // 多播模式下，从 Transport 响应头中提取多播地址和端口
        if self.transport_mode == TransportMode::Multicast {
            if let Some(transport_header) = response.get_header("Transport") {
                info!(target: "rtsp_client", "Multicast Transport: {}", transport_header);
                // 解析示例: Transport: RTP/AVP;multicast;destination=224.0.0.1;port=5000-5001
            }
        }
        
        Ok(response)
    }

    /// 发送 PLAY 请求
    pub async fn play(&mut self) -> Result<RtspResponse> {
        let session = self.session_id.as_ref()
            .ok_or_else(|| anyhow!("No session ID"))?;
        
        let request = format!(
            "PLAY {} RTSP/1.0\r\nCSeq: {}\r\nSession: {}\r\n\r\n",
            self.url, self.cseq, session
        );
        self.cseq += 1;
        
        self.send_request(&request).await
    }

    /// 发送 TEARDOWN 请求
    pub async fn teardown(&mut self) -> Result<RtspResponse> {
        if let Some(session) = &self.session_id {
            let request = format!(
                "TEARDOWN {} RTSP/1.0\r\nCSeq: {}\r\nSession: {}\r\n\r\n",
                self.url, self.cseq, session
            );
            self.cseq += 1;
            
            self.send_request(&request).await
        } else {
            Err(anyhow!("No active session"))
        }
    }

    /// 发送请求并接收响应
    async fn send_request(&mut self, request: &str) -> Result<RtspResponse> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;
        
        debug!(target: "rtsp_client", "Sending request:\n{}", request);
        stream.write_all(request.as_bytes()).await?;
        
        self.receive_response().await
    }

    /// 接收响应
    async fn receive_response(&mut self) -> Result<RtspResponse> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;
        
        let mut reader = BufReader::new(stream);
        let mut lines = Vec::new();
        
        // 读取状态行和头部
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            
            if line == "\r\n" || line == "\n" {
                break;
            }
            
            lines.push(line.trim_end().to_string());
        }
        
        if lines.is_empty() {
            return Err(anyhow!("Empty response"));
        }
        
        // 解析状态行
        let status_line = &lines[0];
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(anyhow!("Invalid status line: {}", status_line));
        }
        
        let status_code = parts[1].parse::<u16>()?;
        let status_text = parts[2..].join(" ");
        
        // 解析头部
        let mut headers = Vec::new();
        let mut content_length = 0;
        
        for line in &lines[1..] {
            if let Some(pos) = line.find(':') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();
                
                if key.eq_ignore_ascii_case("Content-Length") {
                    content_length = value.parse::<usize>().unwrap_or(0);
                }
                
                headers.push((key, value));
            }
        }
        
        // 读取 body
        let mut body = String::new();
        if content_length > 0 {
            let mut buffer = vec![0u8; content_length];
            tokio::io::AsyncReadExt::read_exact(&mut reader, &mut buffer).await?;
            body = String::from_utf8_lossy(&buffer).to_string();
        }
        
        let response = RtspResponse {
            status_code,
            status_text,
            headers,
            body,
        };
        
        debug!(target: "rtsp_client", "Received response: {} {}", response.status_code, response.status_text);
        
        Ok(response)
    }

    /// 启动 Interleaved 数据接收（TCP 模式）
    pub async fn start_interleaved_receiver(
        mut self,
    ) -> Result<(mpsc::Receiver<InterleavedPacket>, mpsc::Receiver<RtspResponse>)> {
        let (data_tx, data_rx) = mpsc::channel(100);
        let (response_tx, response_rx) = mpsc::channel(10);
        
        tokio::spawn(async move {
            if let Err(e) = self.receive_interleaved_loop(data_tx, response_tx).await {
                warn!(target: "rtsp_client", "Interleaved receiver error: {}", e);
            }
        });
        
        Ok((data_rx, response_rx))
    }

    /// Interleaved 接收循环
    async fn receive_interleaved_loop(
        &mut self,
        data_tx: mpsc::Sender<InterleavedPacket>,
        _response_tx: mpsc::Sender<RtspResponse>,
    ) -> Result<()> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;
        
        let mut buffer = vec![0u8; 65536];
        
        loop {
            // 读取第一个字节，判断是 RTSP 响应还是 Interleaved 数据
            let first_byte = match stream.read_u8().await {
                Ok(b) => b,
                Err(_) => break,
            };
            
            if first_byte == b'$' {
                // Interleaved 数据包
                // Format: $ + channel(1) + length(2) + data
                let channel = stream.read_u8().await?;
                let length = stream.read_u16().await? as usize;
                
                if length > buffer.len() {
                    warn!(target: "rtsp_client", "Interleaved packet too large: {}", length);
                    continue;
                }
                
                stream.read_exact(&mut buffer[..length]).await?;
                let data = Bytes::copy_from_slice(&buffer[..length]);
                
                let packet = InterleavedPacket { channel, data };
                
                if data_tx.send(packet).await.is_err() {
                    break;
                }
            } else {
                // RTSP 响应（暂时忽略，因为已经在 send_request 中处理）
                // 这里可以扩展处理异步 RTSP 响应
                debug!(target: "rtsp_client", "Received RTSP response byte: {}", first_byte);
            }
        }
        
        Ok(())
    }

    /// 解析 URL
    fn parse_url(&self) -> Result<ParsedUrl> {
        let url = self.url.strip_prefix("rtsp://")
            .ok_or_else(|| anyhow!("Invalid RTSP URL"))?;
        
        let (host_port, _path) = url.split_once('/')
            .unwrap_or((url, ""));
        
        let (host, port) = if let Some(pos) = host_port.find(':') {
            let host = &host_port[..pos];
            let port = host_port[pos + 1..].parse::<u16>()?;
            (host.to_string(), port)
        } else {
            (host_port.to_string(), 554)
        };
        
        Ok(ParsedUrl { host, port })
    }
}

impl RtspResponse {
    pub fn get_header(&self, key: &str) -> Option<String> {
        self.headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone())
    }
}

struct ParsedUrl {
    host: String,
    port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtsp_client_creation() {
        let client = RtspClient::new("rtsp://192.168.1.100:554/stream1".to_string());
        assert_eq!(client.url, "rtsp://192.168.1.100:554/stream1");
        assert_eq!(client.cseq, 1);
    }

    #[test]
    fn test_parse_url() {
        let client = RtspClient::new("rtsp://192.168.1.100:554/stream1".to_string());
        let parsed = client.parse_url().unwrap();
        assert_eq!(parsed.host, "192.168.1.100");
        assert_eq!(parsed.port, 554);
    }

    #[test]
    fn test_parse_url_default_port() {
        let client = RtspClient::new("rtsp://192.168.1.100/stream1".to_string());
        let parsed = client.parse_url().unwrap();
        assert_eq!(parsed.host, "192.168.1.100");
        assert_eq!(parsed.port, 554);
    }
}
