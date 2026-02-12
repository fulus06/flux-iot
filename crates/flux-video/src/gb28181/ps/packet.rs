// PS 数据包解析
// MPEG-PS (Program Stream) 格式

use bytes::Bytes;

/// PS 包起始码
pub const PS_START_CODE: [u8; 3] = [0x00, 0x00, 0x01];

/// PS 包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsPacketType {
    /// Pack Header (0xBA)
    PackHeader,
    
    /// System Header (0xBB)
    SystemHeader,
    
    /// Program Stream Map (0xBC)
    ProgramStreamMap,
    
    /// Video Stream (0xE0-0xEF)
    Video,
    
    /// Audio Stream (0xC0-0xDF)
    Audio,
    
    /// Unknown
    Unknown,
}

impl PsPacketType {
    pub fn from_stream_id(stream_id: u8) -> Self {
        match stream_id {
            0xBA => Self::PackHeader,
            0xBB => Self::SystemHeader,
            0xBC => Self::ProgramStreamMap,
            0xE0..=0xEF => Self::Video,
            0xC0..=0xDF => Self::Audio,
            _ => Self::Unknown,
        }
    }
}

/// PS 数据包
#[derive(Debug, Clone)]
pub struct PsPacket {
    /// 包类型
    pub packet_type: PsPacketType,
    
    /// 流 ID
    pub stream_id: u8,
    
    /// 数据
    pub data: Bytes,
    
    /// PTS (Presentation Time Stamp)
    pub pts: Option<u64>,
    
    /// DTS (Decoding Time Stamp)
    pub dts: Option<u64>,
}

impl PsPacket {
    /// 查找 PS 起始码
    pub fn find_start_code(data: &[u8], start: usize) -> Option<usize> {
        if start + 3 > data.len() {
            return None;
        }
        
        for i in start..data.len() - 3 {
            if data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x01 {
                return Some(i);
            }
        }
        
        None
    }
    
    /// 解析 PTS/DTS
    pub fn parse_timestamp(data: &[u8], offset: usize) -> Option<u64> {
        if offset + 5 > data.len() {
            return None;
        }
        
        let b0 = data[offset] as u64;
        let b1 = data[offset + 1] as u64;
        let b2 = data[offset + 2] as u64;
        let b3 = data[offset + 3] as u64;
        let b4 = data[offset + 4] as u64;
        
        let pts = ((b0 & 0x0E) << 29)
            | ((b1 & 0xFF) << 22)
            | ((b2 & 0xFE) << 14)
            | ((b3 & 0xFF) << 7)
            | ((b4 & 0xFE) >> 1);
        
        Some(pts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ps_packet_type() {
        assert_eq!(PsPacketType::from_stream_id(0xBA), PsPacketType::PackHeader);
        assert_eq!(PsPacketType::from_stream_id(0xBB), PsPacketType::SystemHeader);
        assert_eq!(PsPacketType::from_stream_id(0xE0), PsPacketType::Video);
        assert_eq!(PsPacketType::from_stream_id(0xC0), PsPacketType::Audio);
    }
    
    #[test]
    fn test_find_start_code() {
        let data = vec![0xFF, 0x00, 0x00, 0x01, 0xBA, 0x00];
        let pos = PsPacket::find_start_code(&data, 0);
        assert_eq!(pos, Some(1));
    }
    
    #[test]
    fn test_parse_timestamp() {
        // 模拟 PTS 数据
        let data = vec![0x21, 0x00, 0x01, 0x00, 0x01];
        let pts = PsPacket::parse_timestamp(&data, 0);
        assert!(pts.is_some());
    }
}
