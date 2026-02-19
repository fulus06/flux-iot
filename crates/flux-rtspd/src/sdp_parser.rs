use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// SDP 会话描述
#[derive(Debug, Clone)]
pub struct SdpSession {
    pub session_name: String,
    pub media_descriptions: Vec<MediaDescription>,
}

/// 媒体描述
#[derive(Debug, Clone)]
pub struct MediaDescription {
    pub media_type: String,  // video, audio
    pub port: u16,
    pub protocol: String,    // RTP/AVP
    pub format: u32,         // payload type
    pub attributes: HashMap<String, String>,
    pub control_url: Option<String>,
}

/// SDP 解析器
pub struct SdpParser;

impl SdpParser {
    /// 解析 SDP
    pub fn parse(sdp: &str) -> Result<SdpSession> {
        let lines: Vec<&str> = sdp.lines().collect();
        
        let mut session_name = String::new();
        let mut media_descriptions = Vec::new();
        let mut current_media: Option<MediaDescription> = None;
        
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            
            if line.len() < 2 || !line.contains('=') {
                continue;
            }
            
            let type_char = &line[0..1];
            let value = &line[2..];
            
            match type_char {
                "s" => {
                    // Session name
                    session_name = value.to_string();
                }
                "m" => {
                    // 保存之前的 media
                    if let Some(media) = current_media.take() {
                        media_descriptions.push(media);
                    }
                    
                    // 解析新的 media 行
                    // m=video 0 RTP/AVP 96
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    if parts.len() >= 4 {
                        current_media = Some(MediaDescription {
                            media_type: parts[0].to_string(),
                            port: parts[1].parse().unwrap_or(0),
                            protocol: parts[2].to_string(),
                            format: parts[3].parse().unwrap_or(0),
                            attributes: HashMap::new(),
                            control_url: None,
                        });
                    }
                }
                "a" => {
                    // 属性行
                    if let Some(ref mut media) = current_media {
                        if let Some(pos) = value.find(':') {
                            let key = &value[..pos];
                            let val = &value[pos + 1..];
                            
                            // 特殊处理 control 属性
                            if key == "control" {
                                media.control_url = Some(val.to_string());
                            }
                            
                            media.attributes.insert(key.to_string(), val.to_string());
                        } else {
                            // 无值属性
                            media.attributes.insert(value.to_string(), String::new());
                        }
                    }
                }
                _ => {
                    // 其他类型暂时忽略
                }
            }
        }
        
        // 保存最后一个 media
        if let Some(media) = current_media {
            media_descriptions.push(media);
        }
        
        Ok(SdpSession {
            session_name,
            media_descriptions,
        })
    }
    
    /// 获取视频轨道
    pub fn get_video_track(session: &SdpSession) -> Option<&MediaDescription> {
        session.media_descriptions.iter()
            .find(|m| m.media_type == "video")
    }
    
    /// 获取音频轨道
    pub fn get_audio_track(session: &SdpSession) -> Option<&MediaDescription> {
        session.media_descriptions.iter()
            .find(|m| m.media_type == "audio")
    }
    
    /// 提取 H264 SPS/PPS
    pub fn extract_h264_params(media: &MediaDescription) -> Option<(Vec<u8>, Vec<u8>)> {
        if let Some(fmtp) = media.attributes.get("fmtp") {
            // fmtp:96 packetization-mode=1;profile-level-id=42C01E;sprop-parameter-sets=Z0LAHtkDxWhAAAADAEAAAAwDxYuS,aMuMsg==
            if let Some(sprop_pos) = fmtp.find("sprop-parameter-sets=") {
                let sprop_start = sprop_pos + "sprop-parameter-sets=".len();
                let sprop_str = &fmtp[sprop_start..];
                let sprop_end = sprop_str.find(';').unwrap_or(sprop_str.len());
                let sprop = &sprop_str[..sprop_end];
                
                let parts: Vec<&str> = sprop.split(',').collect();
                if parts.len() >= 2 {
                    if let (Ok(sps), Ok(pps)) = (
                        base64::decode(parts[0]),
                        base64::decode(parts[1])
                    ) {
                        return Some((sps, pps));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sdp() {
        let sdp = r#"v=0
o=- 0 0 IN IP4 192.168.1.100
s=RTSP Session
c=IN IP4 192.168.1.100
t=0 0
m=video 0 RTP/AVP 96
a=rtpmap:96 H264/90000
a=fmtp:96 packetization-mode=1
a=control:track1
"#;
        
        let session = SdpParser::parse(sdp).unwrap();
        assert_eq!(session.session_name, "RTSP Session");
        assert_eq!(session.media_descriptions.len(), 1);
        
        let video = &session.media_descriptions[0];
        assert_eq!(video.media_type, "video");
        assert_eq!(video.format, 96);
        assert_eq!(video.control_url, Some("track1".to_string()));
    }

    #[test]
    fn test_get_video_track() {
        let sdp = r#"v=0
s=Test
m=video 0 RTP/AVP 96
a=control:track1
m=audio 0 RTP/AVP 97
a=control:track2
"#;
        
        let session = SdpParser::parse(sdp).unwrap();
        let video = SdpParser::get_video_track(&session);
        assert!(video.is_some());
        assert_eq!(video.unwrap().media_type, "video");
    }
}
