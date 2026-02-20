use anyhow::Result;
use bytes::{Buf, Bytes, BytesMut};
use tracing::warn;

use crate::rtp_receiver::RtpPacket;

/// AAC 解包器
pub struct AacDepacketizer {
    last_timestamp: u32,
}

/// AAC 音频帧
#[derive(Debug, Clone)]
pub struct AacFrame {
    pub timestamp: u32,
    pub data: Bytes,
    pub sample_rate: u32,
    pub channels: u8,
}

impl AacDepacketizer {
    pub fn new() -> Self {
        Self {
            last_timestamp: 0,
        }
    }

    /// 处理 RTP 包
    /// RFC 3640 - RTP Payload Format for Transport of MPEG-4 Elementary Streams
    pub fn process_rtp(&mut self, packet: RtpPacket) -> Result<Vec<AacFrame>> {
        let mut frames = Vec::new();
        
        if packet.payload.is_empty() {
            return Ok(frames);
        }
        
        self.last_timestamp = packet.timestamp;
        
        // AAC RTP Payload:
        // +--------+--------+--------+--------+
        // | AU-headers-length (16 bits)      |
        // +--------+--------+--------+--------+
        // | AU-header(1) | AU-header(2) | ...|
        // +--------+--------+--------+--------+
        // | Access Unit 1 data               |
        // +--------+--------+--------+--------+
        // | Access Unit 2 data               |
        // +--------+--------+--------+--------+
        
        let mut cursor = std::io::Cursor::new(&packet.payload);
        
        if cursor.remaining() < 2 {
            return Ok(frames);
        }
        
        // AU-headers-length (in bits)
        let au_headers_length = cursor.get_u16() as usize / 8;
        
        if cursor.remaining() < au_headers_length {
            return Ok(frames);
        }
        
        // 解析 AU-headers
        let mut au_sizes = Vec::new();
        let au_headers_start = cursor.position() as usize;
        let au_headers_end = au_headers_start + au_headers_length;
        
        // 每个 AU-header 通常是 16 bits: AU-size(13 bits) + AU-Index(3 bits)
        while cursor.position() < au_headers_end as u64 {
            if cursor.remaining() < 2 {
                break;
            }
            let au_header = cursor.get_u16();
            let au_size = (au_header >> 3) as usize; // 高 13 bits
            au_sizes.push(au_size);
        }
        
        // 读取 Access Units
        for au_size in au_sizes {
            if cursor.remaining() < au_size {
                break;
            }
            
            let start = cursor.position() as usize;
            let end = start + au_size;
            let au_data = Bytes::copy_from_slice(&packet.payload[start..end]);
            
            frames.push(AacFrame {
                timestamp: packet.timestamp,
                data: au_data,
                sample_rate: 48000, // 默认，应从 SDP 中获取
                channels: 2,        // 默认立体声
            });
            
            cursor.set_position(end as u64);
        }
        
        Ok(frames)
    }

    /// 从 SDP fmtp 解析 AAC 配置
    /// 例如: "profile-level-id=1;mode=AAC-hbr;sizelength=13;indexlength=3;indexdeltalength=3;config=1190"
    pub fn parse_config(fmtp: &str) -> Option<AacConfig> {
        let mut config = AacConfig::default();
        
        for param in fmtp.split(';') {
            let parts: Vec<&str> = param.split('=').collect();
            if parts.len() != 2 {
                continue;
            }
            
            let key = parts[0].trim();
            let value = parts[1].trim();
            
            match key {
                "config" => {
                    // config 是十六进制的 AudioSpecificConfig
                    if let Ok(config_bytes) = hex::decode(value) {
                        if let Some(parsed) = Self::parse_audio_specific_config(&config_bytes[..]) {
                            config.sample_rate = parsed.sample_rate;
                            config.channels = parsed.channels;
                        }
                    }
                }
                "sizelength" => {
                    config.size_length = value.parse().unwrap_or(13);
                }
                "indexlength" => {
                    config.index_length = value.parse().unwrap_or(3);
                }
                _ => {}
            }
        }
        
        Some(config)
    }

    /// 解析 AudioSpecificConfig
    /// ISO/IEC 14496-3 Section 1.6.2.1
    fn parse_audio_specific_config(data: &[u8]) -> Option<AacConfig> {
        if data.is_empty() {
            return None;
        }
        
        // AudioSpecificConfig:
        // audioObjectType (5 bits)
        // samplingFrequencyIndex (4 bits)
        // channelConfiguration (4 bits)
        
        let byte0 = data[0];
        let byte1 = if data.len() > 1 { data[1] } else { 0 };
        
        let _audio_object_type = byte0 >> 3;
        let sampling_frequency_index = ((byte0 & 0x07) << 1) | (byte1 >> 7);
        let channel_configuration = (byte1 >> 3) & 0x0F;
        
        let sample_rate = match sampling_frequency_index {
            0x0 => 96000,
            0x1 => 88200,
            0x2 => 64000,
            0x3 => 48000,
            0x4 => 44100,
            0x5 => 32000,
            0x6 => 24000,
            0x7 => 22050,
            0x8 => 16000,
            0x9 => 12000,
            0xa => 11025,
            0xb => 8000,
            0xc => 7350,
            _ => 48000, // 默认
        };
        
        Some(AacConfig {
            sample_rate,
            channels: channel_configuration,
            size_length: 13,
            index_length: 3,
        })
    }
}

/// AAC 配置
#[derive(Debug, Clone)]
pub struct AacConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub size_length: u8,
    pub index_length: u8,
}

impl Default for AacConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            size_length: 13,
            index_length: 3,
        }
    }
}

impl Default for AacDepacketizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_audio_specific_config() {
        // AAC-LC, 48kHz, Stereo
        // 0x1190 = 0001 0001 1001 0000
        // audioObjectType = 2 (AAC-LC)
        // samplingFrequencyIndex = 3 (48kHz)
        // channelConfiguration = 2 (Stereo)
        let config_bytes = hex::decode("1190").unwrap();
        let config = AacDepacketizer::parse_audio_specific_config(&config_bytes).unwrap();
        
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
    }

    #[test]
    fn test_process_rtp_single_au() {
        let mut depacketizer = AacDepacketizer::new();
        
        // 构造一个简单的 AAC RTP 包
        // AU-headers-length = 16 bits (0x0010)
        // AU-header = 0x0050 (size=10, index=0)
        // AU data = 10 bytes
        let mut payload = BytesMut::new();
        payload.extend_from_slice(&[0x00, 0x10]); // AU-headers-length = 16 bits
        payload.extend_from_slice(&[0x00, 0x50]); // AU-header: size=10
        payload.extend_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A]); // AU data
        
        let packet = RtpPacket {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: true,
            payload_type: 97,
            sequence_number: 1,
            timestamp: 1000,
            ssrc: 0x12345678,
            payload: payload.freeze(),
        };
        
        let frames = depacketizer.process_rtp(packet).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].timestamp, 1000);
        assert_eq!(frames[0].data.len(), 10);
    }
}
