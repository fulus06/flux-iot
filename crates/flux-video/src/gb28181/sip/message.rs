// SIP 消息解析和生成
// 支持 GB28181 标准的 SIP 消息格式

use std::collections::HashMap;
use std::fmt;

/// SIP 方法
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SipMethod {
    Register,
    Invite,
    Ack,
    Bye,
    Cancel,
    Message,
    Subscribe,
    Notify,
    Info,
}

impl fmt::Display for SipMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SipMethod::Register => write!(f, "REGISTER"),
            SipMethod::Invite => write!(f, "INVITE"),
            SipMethod::Ack => write!(f, "ACK"),
            SipMethod::Bye => write!(f, "BYE"),
            SipMethod::Cancel => write!(f, "CANCEL"),
            SipMethod::Message => write!(f, "MESSAGE"),
            SipMethod::Subscribe => write!(f, "SUBSCRIBE"),
            SipMethod::Notify => write!(f, "NOTIFY"),
            SipMethod::Info => write!(f, "INFO"),
        }
    }
}

impl SipMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "REGISTER" => Some(SipMethod::Register),
            "INVITE" => Some(SipMethod::Invite),
            "ACK" => Some(SipMethod::Ack),
            "BYE" => Some(SipMethod::Bye),
            "CANCEL" => Some(SipMethod::Cancel),
            "MESSAGE" => Some(SipMethod::Message),
            "SUBSCRIBE" => Some(SipMethod::Subscribe),
            "NOTIFY" => Some(SipMethod::Notify),
            "INFO" => Some(SipMethod::Info),
            _ => None,
        }
    }
}

/// SIP 请求
#[derive(Debug, Clone)]
pub struct SipRequest {
    pub method: SipMethod,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl SipRequest {
    pub fn new(method: SipMethod, uri: String) -> Self {
        Self {
            method,
            uri,
            version: "SIP/2.0".to_string(),
            headers: HashMap::new(),
            body: None,
        }
    }
    
    /// 添加头部
    pub fn add_header(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }
    
    /// 设置消息体
    pub fn set_body(&mut self, body: String) {
        self.body = Some(body);
    }
    
    /// 生成 SIP 请求字符串
    pub fn to_string(&self) -> String {
        let mut result = format!("{} {} {}\r\n", self.method, self.uri, self.version);
        
        // 添加头部
        for (key, value) in &self.headers {
            result.push_str(&format!("{}: {}\r\n", key, value));
        }
        
        // 如果有消息体，添加 Content-Length
        if let Some(body) = &self.body {
            result.push_str(&format!("Content-Length: {}\r\n", body.len()));
            result.push_str("\r\n");
            result.push_str(body);
        } else {
            result.push_str("Content-Length: 0\r\n");
            result.push_str("\r\n");
        }
        
        result
    }
    
    /// 从字符串解析 SIP 请求
    pub fn from_string(s: &str) -> Result<Self, String> {
        let lines: Vec<&str> = s.split("\r\n").collect();
        
        if lines.is_empty() {
            return Err("Empty SIP message".to_string());
        }
        
        // 解析请求行
        let request_line: Vec<&str> = lines[0].split_whitespace().collect();
        if request_line.len() != 3 {
            return Err("Invalid request line".to_string());
        }
        
        let method = SipMethod::from_str(request_line[0])
            .ok_or_else(|| format!("Unknown method: {}", request_line[0]))?;
        let uri = request_line[1].to_string();
        let version = request_line[2].to_string();
        
        // 解析头部
        let mut headers = HashMap::new();
        let mut body_start = 0;
        
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.is_empty() {
                body_start = i + 1;
                break;
            }
            
            if let Some(pos) = line.find(':') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }
        
        // 解析消息体
        let body = if body_start < lines.len() {
            let body_lines: Vec<&str> = lines[body_start..].iter().copied().collect();
            let body_str = body_lines.join("\r\n");
            if body_str.is_empty() {
                None
            } else {
                Some(body_str)
            }
        } else {
            None
        };
        
        Ok(Self {
            method,
            uri,
            version,
            headers,
            body,
        })
    }
}

/// SIP 响应
#[derive(Debug, Clone)]
pub struct SipResponse {
    pub version: String,
    pub status_code: u16,
    pub reason_phrase: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl SipResponse {
    pub fn new(status_code: u16, reason_phrase: String) -> Self {
        Self {
            version: "SIP/2.0".to_string(),
            status_code,
            reason_phrase,
            headers: HashMap::new(),
            body: None,
        }
    }
    
    /// 添加头部
    pub fn add_header(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }
    
    /// 设置消息体
    pub fn set_body(&mut self, body: String) {
        self.body = Some(body);
    }
    
    /// 生成 SIP 响应字符串
    pub fn to_string(&self) -> String {
        let mut result = format!("{} {} {}\r\n", self.version, self.status_code, self.reason_phrase);
        
        // 添加头部
        for (key, value) in &self.headers {
            result.push_str(&format!("{}: {}\r\n", key, value));
        }
        
        // 如果有消息体，添加 Content-Length
        if let Some(body) = &self.body {
            result.push_str(&format!("Content-Length: {}\r\n", body.len()));
            result.push_str("\r\n");
            result.push_str(body);
        } else {
            result.push_str("Content-Length: 0\r\n");
            result.push_str("\r\n");
        }
        
        result
    }
    
    /// 从字符串解析 SIP 响应
    pub fn from_string(s: &str) -> Result<Self, String> {
        let lines: Vec<&str> = s.split("\r\n").collect();
        
        if lines.is_empty() {
            return Err("Empty SIP message".to_string());
        }
        
        // 解析状态行
        let status_line: Vec<&str> = lines[0].splitn(3, ' ').collect();
        if status_line.len() != 3 {
            return Err("Invalid status line".to_string());
        }
        
        let version = status_line[0].to_string();
        let status_code = status_line[1].parse::<u16>()
            .map_err(|_| "Invalid status code".to_string())?;
        let reason_phrase = status_line[2].to_string();
        
        // 解析头部
        let mut headers = HashMap::new();
        let mut body_start = 0;
        
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.is_empty() {
                body_start = i + 1;
                break;
            }
            
            if let Some(pos) = line.find(':') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }
        
        // 解析消息体
        let body = if body_start < lines.len() {
            let body_lines: Vec<&str> = lines[body_start..].iter().copied().collect();
            let body_str = body_lines.join("\r\n");
            if body_str.is_empty() {
                None
            } else {
                Some(body_str)
            }
        } else {
            None
        };
        
        Ok(Self {
            version,
            status_code,
            reason_phrase,
            headers,
            body,
        })
    }
}

/// SIP 消息（请求或响应）
#[derive(Debug, Clone)]
pub enum SipMessage {
    Request(SipRequest),
    Response(SipResponse),
}

impl SipMessage {
    /// 从字符串解析 SIP 消息
    pub fn from_string(s: &str) -> Result<Self, String> {
        if s.starts_with("SIP/") {
            Ok(SipMessage::Response(SipResponse::from_string(s)?))
        } else {
            Ok(SipMessage::Request(SipRequest::from_string(s)?))
        }
    }
    
    /// 转换为字符串
    pub fn to_string(&self) -> String {
        match self {
            SipMessage::Request(req) => req.to_string(),
            SipMessage::Response(resp) => resp.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sip_request_generation() {
        let mut req = SipRequest::new(
            SipMethod::Register,
            "sip:34020000002000000001@192.168.1.100:5060".to_string()
        );
        
        req.add_header("Via".to_string(), "SIP/2.0/UDP 192.168.1.100:5060".to_string());
        req.add_header("From".to_string(), "<sip:34020000002000000001@192.168.1.100:5060>".to_string());
        req.add_header("To".to_string(), "<sip:34020000002000000001@192.168.1.100:5060>".to_string());
        req.add_header("Call-ID".to_string(), "123456789@192.168.1.100".to_string());
        req.add_header("CSeq".to_string(), "1 REGISTER".to_string());
        
        let sip_str = req.to_string();
        
        assert!(sip_str.contains("REGISTER"));
        assert!(sip_str.contains("Via:"));
        assert!(sip_str.contains("Content-Length: 0"));
    }
    
    #[test]
    fn test_sip_request_parsing() {
        let sip_str = "REGISTER sip:34020000002000000001@192.168.1.100:5060 SIP/2.0\r\n\
                       Via: SIP/2.0/UDP 192.168.1.100:5060\r\n\
                       From: <sip:34020000002000000001@192.168.1.100:5060>\r\n\
                       To: <sip:34020000002000000001@192.168.1.100:5060>\r\n\
                       Call-ID: 123456789@192.168.1.100\r\n\
                       CSeq: 1 REGISTER\r\n\
                       Content-Length: 0\r\n\
                       \r\n";
        
        let req = SipRequest::from_string(sip_str).unwrap();
        
        assert_eq!(req.method, SipMethod::Register);
        assert_eq!(req.uri, "sip:34020000002000000001@192.168.1.100:5060");
        assert!(req.headers.contains_key("Via"));
        assert!(req.headers.contains_key("From"));
    }
    
    #[test]
    fn test_sip_response_generation() {
        let mut resp = SipResponse::new(200, "OK".to_string());
        
        resp.add_header("Via".to_string(), "SIP/2.0/UDP 192.168.1.100:5060".to_string());
        resp.add_header("From".to_string(), "<sip:34020000002000000001@192.168.1.100:5060>".to_string());
        resp.add_header("To".to_string(), "<sip:34020000002000000001@192.168.1.100:5060>".to_string());
        
        let sip_str = resp.to_string();
        
        assert!(sip_str.contains("SIP/2.0 200 OK"));
        assert!(sip_str.contains("Via:"));
    }
}
