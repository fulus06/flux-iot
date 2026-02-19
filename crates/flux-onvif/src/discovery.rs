use anyhow::{anyhow, Result};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;
use tracing::{debug, info};
use uuid::Uuid;

/// ONVIF 设备信息
#[derive(Debug, Clone)]
pub struct OnvifDevice {
    pub uuid: String,
    pub name: String,
    pub hardware: String,
    pub location: String,
    pub service_url: String,
    pub scopes: Vec<String>,
}

/// ONVIF 设备发现
pub struct OnvifDiscovery {
    socket: UdpSocket,
    timeout: Duration,
}

impl OnvifDiscovery {
    /// 创建发现服务
    pub fn new() -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_broadcast(true)?;
        
        Ok(Self {
            socket,
            timeout: Duration::from_secs(5),
        })
    }

    /// 发现设备
    pub fn discover(&self) -> Result<Vec<OnvifDevice>> {
        let probe_message = self.build_probe_message();
        
        // 发送到多播地址
        let multicast_addr: SocketAddr = "239.255.255.250:3702".parse()?;
        self.socket.send_to(probe_message.as_bytes(), multicast_addr)?;
        
        info!(target: "onvif_discovery", "Sent WS-Discovery probe");
        
        // 接收响应
        let mut devices = Vec::new();
        let mut buffer = vec![0u8; 8192];
        let start = std::time::Instant::now();
        
        while start.elapsed() < self.timeout {
            match self.socket.recv_from(&mut buffer) {
                Ok((len, addr)) => {
                    let response = String::from_utf8_lossy(&buffer[..len]);
                    debug!(target: "onvif_discovery", "Received response from {}", addr);
                    
                    if let Ok(device) = self.parse_probe_match(&response) {
                        info!(target: "onvif_discovery", "Found device: {}", device.name);
                        devices.push(device);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // 超时，继续等待
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    debug!(target: "onvif_discovery", "Error receiving: {}", e);
                    break;
                }
            }
        }
        
        Ok(devices)
    }

    /// 构建 WS-Discovery Probe 消息
    fn build_probe_message(&self) -> String {
        let uuid = Uuid::new_v4();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing">
  <s:Header>
    <a:Action s:mustUnderstand="1">http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</a:Action>
    <a:MessageID>uuid:{}</a:MessageID>
    <a:ReplyTo>
      <a:Address>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</a:Address>
    </a:ReplyTo>
    <a:To s:mustUnderstand="1">urn:schemas-xmlsoap-org:ws:2005:04:discovery</a:To>
  </s:Header>
  <s:Body>
    <Probe xmlns="http://schemas.xmlsoap.org/ws/2005/04/discovery">
      <d:Types xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:dp0="http://www.onvif.org/ver10/network/wsdl">dp0:NetworkVideoTransmitter</d:Types>
    </Probe>
  </s:Body>
</s:Envelope>"#,
            uuid
        )
    }

    /// 解析 ProbeMatch 响应
    fn parse_probe_match(&self, xml: &str) -> Result<OnvifDevice> {
        // 简化的 XML 解析（生产环境应使用 quick-xml）
        let uuid = Self::extract_tag(xml, "Address")?;
        let service_url = Self::extract_tag(xml, "XAddrs")?;
        let scopes_str = Self::extract_tag(xml, "Scopes").unwrap_or_default();
        
        // 解析 scopes
        let scopes: Vec<String> = scopes_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        
        // 从 scopes 中提取信息
        let name = Self::extract_scope_value(&scopes, "name").unwrap_or_else(|| "Unknown".to_string());
        let hardware = Self::extract_scope_value(&scopes, "hardware").unwrap_or_else(|| "Unknown".to_string());
        let location = Self::extract_scope_value(&scopes, "location").unwrap_or_else(|| "Unknown".to_string());
        
        Ok(OnvifDevice {
            uuid,
            name,
            hardware,
            location,
            service_url,
            scopes,
        })
    }

    /// 提取 XML 标签内容
    fn extract_tag(xml: &str, tag: &str) -> Result<String> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);
        
        if let Some(start) = xml.find(&start_tag) {
            let content_start = start + start_tag.len();
            if let Some(end) = xml[content_start..].find(&end_tag) {
                return Ok(xml[content_start..content_start + end].to_string());
            }
        }
        
        // 尝试自闭合标签或带命名空间的标签
        let patterns = [
            format!("<{}:", tag),
            format!("<a:{}>", tag),
            format!("<d:{}>", tag),
        ];
        
        for pattern in &patterns {
            if let Some(start) = xml.find(pattern) {
                let content_start = xml[start..].find('>').map(|i| start + i + 1);
                if let Some(cs) = content_start {
                    if let Some(end) = xml[cs..].find('<') {
                        return Ok(xml[cs..cs + end].to_string());
                    }
                }
            }
        }
        
        Err(anyhow!("Tag {} not found", tag))
    }

    /// 从 scopes 中提取值
    fn extract_scope_value(scopes: &[String], key: &str) -> Option<String> {
        for scope in scopes {
            if let Some(pos) = scope.find(&format!("/{}/", key)) {
                let value_start = pos + key.len() + 2;
                let value = scope[value_start..].split('/').next()?;
                return Some(value.to_string());
            }
        }
        None
    }
}

impl Default for OnvifDiscovery {
    fn default() -> Self {
        Self::new().expect("Failed to create OnvifDiscovery")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tag() {
        let xml = r#"<Address>uuid:12345</Address>"#;
        let result = OnvifDiscovery::extract_tag(xml, "Address").unwrap();
        assert_eq!(result, "uuid:12345");
    }

    #[test]
    fn test_extract_scope_value() {
        let scopes = vec![
            "onvif://www.onvif.org/name/Camera1".to_string(),
            "onvif://www.onvif.org/hardware/IPC".to_string(),
        ];
        
        let name = OnvifDiscovery::extract_scope_value(&scopes, "name");
        assert_eq!(name, Some("Camera1".to_string()));
        
        let hardware = OnvifDiscovery::extract_scope_value(&scopes, "hardware");
        assert_eq!(hardware, Some("IPC".to_string()));
    }

    #[test]
    fn test_build_probe_message() {
        let discovery = OnvifDiscovery::new().unwrap();
        let message = discovery.build_probe_message();
        
        assert!(message.contains("Probe"));
        assert!(message.contains("NetworkVideoTransmitter"));
    }
}
