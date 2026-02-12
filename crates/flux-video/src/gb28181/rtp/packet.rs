// RTP 数据包解析
// RFC 3550 - RTP: A Transport Protocol for Real-Time Applications

use bytes::Bytes;

/// RTP 头部
#[derive(Debug, Clone)]
pub struct RtpHeader {
    /// 版本（2 bits）
    pub version: u8,
    
    /// 填充标志（1 bit）
    pub padding: bool,
    
    /// 扩展标志（1 bit）
    pub extension: bool,
    
    /// CSRC 计数（4 bits）
    pub csrc_count: u8,
    
    /// 标记位（1 bit）
    pub marker: bool,
    
    /// 负载类型（7 bits）
    pub payload_type: u8,
    
    /// 序列号（16 bits）
    pub sequence: u16,
    
    /// 时间戳（32 bits）
    pub timestamp: u32,
    
    /// SSRC（32 bits）
    pub ssrc: u32,
}

impl RtpHeader {
    /// 从字节数组解析 RTP 头部
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }
        
        let byte0 = data[0];
        let byte1 = data[1];
        
        let version = (byte0 >> 6) & 0x03;
        let padding = (byte0 & 0x20) != 0;
        let extension = (byte0 & 0x10) != 0;
        let csrc_count = byte0 & 0x0F;
        
        let marker = (byte1 & 0x80) != 0;
        let payload_type = byte1 & 0x7F;
        
        let sequence = u16::from_be_bytes([data[2], data[3]]);
        let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        
        Some(Self {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence,
            timestamp,
            ssrc,
        })
    }
    
    /// 获取头部长度（字节）
    pub fn header_len(&self) -> usize {
        12 + (self.csrc_count as usize * 4)
    }
}

/// RTP 数据包
#[derive(Debug, Clone)]
pub struct RtpPacket {
    /// RTP 头部
    pub header: RtpHeader,
    
    /// 负载数据
    pub payload: Bytes,
}

impl RtpPacket {
    /// 从字节数组解析 RTP 数据包
    pub fn from_bytes(data: Bytes) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }
        
        let header = RtpHeader::from_bytes(&data)?;
        let header_len = header.header_len();
        
        if data.len() < header_len {
            return None;
        }
        
        let payload = data.slice(header_len..);
        
        Some(Self { header, payload })
    }
    
    /// 获取序列号
    pub fn sequence(&self) -> u16 {
        self.header.sequence
    }
    
    /// 获取时间戳
    pub fn timestamp(&self) -> u32 {
        self.header.timestamp
    }
    
    /// 获取负载类型
    pub fn payload_type(&self) -> u8 {
        self.header.payload_type
    }
    
    /// 是否为标记包（通常表示帧结束）
    pub fn is_marker(&self) -> bool {
        self.header.marker
    }
    
    /// 获取负载数据
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rtp_header_parsing() {
        // 构造一个 RTP 头部
        let data = vec![
            0x80, // V=2, P=0, X=0, CC=0
            0x60, // M=0, PT=96
            0x00, 0x01, // Sequence = 1
            0x00, 0x00, 0x00, 0x64, // Timestamp = 100
            0x12, 0x34, 0x56, 0x78, // SSRC
        ];
        
        let header = RtpHeader::from_bytes(&data).unwrap();
        
        assert_eq!(header.version, 2);
        assert_eq!(header.padding, false);
        assert_eq!(header.extension, false);
        assert_eq!(header.csrc_count, 0);
        assert_eq!(header.marker, false);
        assert_eq!(header.payload_type, 96);
        assert_eq!(header.sequence, 1);
        assert_eq!(header.timestamp, 100);
        assert_eq!(header.ssrc, 0x12345678);
    }
    
    #[test]
    fn test_rtp_packet_parsing() {
        let mut data = vec![
            0x80, 0x60, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x64,
            0x12, 0x34, 0x56, 0x78,
        ];
        
        // 添加负载数据
        data.extend_from_slice(b"Hello RTP");
        
        let packet = RtpPacket::from_bytes(Bytes::from(data)).unwrap();
        
        assert_eq!(packet.sequence(), 1);
        assert_eq!(packet.timestamp(), 100);
        assert_eq!(packet.payload_type(), 96);
        assert_eq!(packet.payload().len(), 9);
        assert_eq!(&packet.payload()[..], b"Hello RTP");
    }
    
    #[test]
    fn test_rtp_marker_bit() {
        let data = vec![
            0x80, 0xE0, // M=1, PT=96
            0x00, 0x01,
            0x00, 0x00, 0x00, 0x64,
            0x12, 0x34, 0x56, 0x78,
        ];
        
        let header = RtpHeader::from_bytes(&data).unwrap();
        assert_eq!(header.marker, true);
    }
}
