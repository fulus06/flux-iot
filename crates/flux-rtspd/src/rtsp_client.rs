use anyhow::{anyhow, Result};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, info};

/// RTSP 客户端
pub struct RtspClient {
    stream: Option<TcpStream>,
    url: String,
    session_id: Option<String>,
    cseq: u32,
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
        }
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
        let request = format!(
            "SETUP {} RTSP/1.0\r\nCSeq: {}\r\nTransport: RTP/AVP;unicast;client_port={}-{}\r\n\r\n",
            track_url, self.cseq, client_port, client_port + 1
        );
        self.cseq += 1;
        
        let response = self.send_request(&request).await?;
        
        // 提取 Session ID
        if let Some(session) = response.get_header("Session") {
            self.session_id = Some(session.split(';').next().unwrap_or(&session).to_string());
            info!(target: "rtsp_client", "Session ID: {}", session);
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
