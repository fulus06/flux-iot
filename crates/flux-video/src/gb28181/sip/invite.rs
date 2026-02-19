// GB28181 实时点播控制
// 处理 INVITE/ACK/BYE 等实时流控制

use crate::Result;

/// SDP 会话描述
#[derive(Debug, Clone)]
pub struct SdpSession {
    /// 版本
    pub version: u8,
    
    /// 会话 ID
    pub session_id: String,
    
    /// 会话名称
    pub session_name: String,
    
    /// 连接信息
    pub connection: SdpConnection,

    /// SSRC（GB28181 使用 y= 行携带 SSRC）
    pub ssrc: Option<u32>,
    
    /// 媒体描述
    pub media: Vec<SdpMedia>,
}

/// SDP 连接信息
#[derive(Debug, Clone)]
pub struct SdpConnection {
    /// 网络类型（IN）
    pub network_type: String,
    
    /// 地址类型（IP4/IP6）
    pub address_type: String,
    
    /// IP 地址
    pub address: String,
}

/// SDP 媒体描述
#[derive(Debug, Clone)]
pub struct SdpMedia {
    /// 媒体类型（video/audio）
    pub media_type: String,
    
    /// 端口
    pub port: u16,
    
    /// 传输协议（RTP/AVP）
    pub protocol: String,
    
    /// 格式列表
    pub formats: Vec<u8>,
    
    /// RTP 映射
    pub rtpmap: Vec<RtpMap>,
    
    /// 属性
    pub attributes: Vec<String>,
}

/// RTP 映射
#[derive(Debug, Clone)]
pub struct RtpMap {
    /// 负载类型
    pub payload_type: u8,
    
    /// 编码名称（PS/H264/MPEG4）
    pub encoding_name: String,
    
    /// 时钟频率
    pub clock_rate: u32,
}

impl SdpSession {
    /// 创建新的 SDP 会话
    pub fn new(session_id: String, ip: String) -> Self {
        Self {
            version: 0,
            session_id: session_id.clone(),
            session_name: "Play".to_string(),
            connection: SdpConnection {
                network_type: "IN".to_string(),
                address_type: "IP4".to_string(),
                address: ip,
            },
            ssrc: None,
            media: Vec::new(),
        }
    }
    
    /// 添加视频媒体
    pub fn add_video(&mut self, port: u16) {
        let mut media = SdpMedia {
            media_type: "video".to_string(),
            port,
            protocol: "RTP/AVP".to_string(),
            formats: vec![96, 98, 97],
            rtpmap: vec![
                RtpMap {
                    payload_type: 96,
                    encoding_name: "PS".to_string(),
                    clock_rate: 90000,
                },
                RtpMap {
                    payload_type: 98,
                    encoding_name: "H264".to_string(),
                    clock_rate: 90000,
                },
                RtpMap {
                    payload_type: 97,
                    encoding_name: "MPEG4".to_string(),
                    clock_rate: 90000,
                },
            ],
            attributes: vec!["recvonly".to_string()],
        };
        
        self.media.push(media);
    }
    
    /// 生成 SDP 字符串
    pub fn to_string(&self) -> String {
        let mut sdp = String::new();
        
        // v= 版本
        sdp.push_str(&format!("v={}\r\n", self.version));
        
        // o= 会话源
        sdp.push_str(&format!(
            "o={} 0 0 {} {} {}\r\n",
            self.session_id,
            self.connection.network_type,
            self.connection.address_type,
            self.connection.address
        ));
        
        // s= 会话名称
        sdp.push_str(&format!("s={}\r\n", self.session_name));
        
        // c= 连接信息
        sdp.push_str(&format!(
            "c={} {} {}\r\n",
            self.connection.network_type,
            self.connection.address_type,
            self.connection.address
        ));
        
        // t= 时间
        sdp.push_str("t=0 0\r\n");

        // y= SSRC (GB28181)
        if let Some(ssrc) = self.ssrc {
            sdp.push_str(&format!("y={:010}\r\n", ssrc));
        }
        
        // m= 媒体描述
        for media in &self.media {
            let formats: Vec<String> = media.formats.iter().map(|f| f.to_string()).collect();
            sdp.push_str(&format!(
                "m={} {} {} {}\r\n",
                media.media_type,
                media.port,
                media.protocol,
                formats.join(" ")
            ));
            
            // a=rtpmap
            for rtpmap in &media.rtpmap {
                sdp.push_str(&format!(
                    "a=rtpmap:{} {}/{}\r\n",
                    rtpmap.payload_type,
                    rtpmap.encoding_name,
                    rtpmap.clock_rate
                ));
            }
            
            // a= 其他属性
            for attr in &media.attributes {
                sdp.push_str(&format!("a={}\r\n", attr));
            }
        }
        
        sdp
    }
    
    /// 从字符串解析 SDP
    pub fn from_string(sdp: &str) -> Result<Self> {
        let lines: Vec<&str> = sdp.lines().collect();
        
        let mut session = SdpSession {
            version: 0,
            session_id: String::new(),
            session_name: String::new(),
            connection: SdpConnection {
                network_type: "IN".to_string(),
                address_type: "IP4".to_string(),
                address: String::new(),
            },
            ssrc: None,
            media: Vec::new(),
        };
        
        let mut current_media: Option<SdpMedia> = None;
        
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            
            if let Some(pos) = line.find('=') {
                let key = &line[..pos];
                let value = &line[pos + 1..];
                
                match key {
                    "v" => {
                        session.version = value.parse().unwrap_or(0);
                    }
                    "o" => {
                        let parts: Vec<&str> = value.split_whitespace().collect();
                        if !parts.is_empty() {
                            session.session_id = parts[0].to_string();
                        }
                    }
                    "s" => {
                        session.session_name = value.to_string();
                    }
                    "c" => {
                        let parts: Vec<&str> = value.split_whitespace().collect();
                        if parts.len() >= 3 {
                            session.connection.network_type = parts[0].to_string();
                            session.connection.address_type = parts[1].to_string();
                            session.connection.address = parts[2].to_string();
                        }
                    }
                    "y" => {
                        if let Ok(v) = value.trim().parse::<u32>() {
                            session.ssrc = Some(v);
                        }
                    }
                    "m" => {
                        // 保存之前的媒体
                        if let Some(media) = current_media.take() {
                            session.media.push(media);
                        }
                        
                        // 解析新的媒体描述
                        let parts: Vec<&str> = value.split_whitespace().collect();
                        if parts.len() >= 4 {
                            let formats: Vec<u8> = parts[3..]
                                .iter()
                                .filter_map(|s| s.parse().ok())
                                .collect();
                            
                            current_media = Some(SdpMedia {
                                media_type: parts[0].to_string(),
                                port: parts[1].parse().unwrap_or(0),
                                protocol: parts[2].to_string(),
                                formats,
                                rtpmap: Vec::new(),
                                attributes: Vec::new(),
                            });
                        }
                    }
                    "a" => {
                        if let Some(ref mut media) = current_media {
                            if value.starts_with("rtpmap:") {
                                // 解析 rtpmap
                                let rtpmap_str = &value[7..];
                                if let Some(space_pos) = rtpmap_str.find(' ') {
                                    let payload_type = rtpmap_str[..space_pos].parse().unwrap_or(0);
                                    let rest = &rtpmap_str[space_pos + 1..];
                                    
                                    if let Some(slash_pos) = rest.find('/') {
                                        let encoding_name = rest[..slash_pos].to_string();
                                        let clock_rate = rest[slash_pos + 1..].parse().unwrap_or(90000);
                                        
                                        media.rtpmap.push(RtpMap {
                                            payload_type,
                                            encoding_name,
                                            clock_rate,
                                        });
                                    }
                                }
                            } else {
                                media.attributes.push(value.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        
        // 保存最后一个媒体
        if let Some(media) = current_media {
            session.media.push(media);
        }
        
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sdp_generation() {
        let mut session = SdpSession::new("34020000002000000001".to_string(), "192.168.1.100".to_string());
        session.ssrc = Some(123);
        session.add_video(9000);
        
        let sdp = session.to_string();
        
        assert!(sdp.contains("v=0"));
        assert!(sdp.contains("o=34020000002000000001"));
        assert!(sdp.contains("s=Play"));
        assert!(sdp.contains("c=IN IP4 192.168.1.100"));
        assert!(sdp.contains("m=video 9000 RTP/AVP 96 98 97"));
        assert!(sdp.contains("a=rtpmap:96 PS/90000"));
        assert!(sdp.contains("a=rtpmap:98 H264/90000"));
        assert!(sdp.contains("a=recvonly"));
        assert!(sdp.contains("y=0000000123"));
    }
    
    #[test]
    fn test_sdp_parsing() {
        let sdp_str = "v=0\r\n\
                       o=34020000001320000001 0 0 IN IP4 192.168.1.200\r\n\
                       s=Play\r\n\
                       c=IN IP4 192.168.1.200\r\n\
                       t=0 0\r\n\
                       y=0000001234\r\n\
                       m=video 15060 RTP/AVP 96\r\n\
                       a=rtpmap:96 PS/90000\r\n\
                       a=sendonly\r\n";
        
        let session = SdpSession::from_string(sdp_str).unwrap();
        
        assert_eq!(session.version, 0);
        assert_eq!(session.session_id, "34020000001320000001");
        assert_eq!(session.connection.address, "192.168.1.200");
        assert_eq!(session.ssrc, Some(1234));
        assert_eq!(session.media.len(), 1);
        
        let media = &session.media[0];
        assert_eq!(media.media_type, "video");
        assert_eq!(media.port, 15060);
        assert_eq!(media.rtpmap.len(), 1);
        assert_eq!(media.rtpmap[0].encoding_name, "PS");
    }
}
