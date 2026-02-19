use crate::error::Result;
use bytes::{BufMut, Bytes, BytesMut};

/// FLV 封装器
pub struct FlvMuxer {
    has_sent_header: bool,
}

/// FLV 标签类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlvTagType {
    Audio = 8,
    Video = 9,
    Script = 18,
}

/// FLV 标签
#[derive(Debug, Clone)]
pub struct FlvTag {
    pub tag_type: FlvTagType,
    pub timestamp: u32,
    pub data: Bytes,
}

impl FlvMuxer {
    pub fn new() -> Self {
        Self {
            has_sent_header: false,
        }
    }

    /// 生成 FLV 文件头
    pub fn generate_header(&mut self) -> Bytes {
        self.has_sent_header = true;

        let mut header = BytesMut::with_capacity(13);
        
        // FLV signature
        header.put_slice(b"FLV");
        
        // Version
        header.put_u8(1);
        
        // Flags (audio + video)
        header.put_u8(0x05); // 0000 0101 (audio + video)
        
        // Data offset
        header.put_u32(9);
        
        // Previous tag size (always 0 for first tag)
        header.put_u32(0);

        header.freeze()
    }

    /// 封装 FLV 标签
    pub fn mux_tag(&self, tag: &FlvTag) -> Result<Bytes> {
        let data_size = tag.data.len();
        let mut buffer = BytesMut::with_capacity(11 + data_size + 4);

        // Tag type
        buffer.put_u8(tag.tag_type as u8);

        // Data size (24-bit)
        buffer.put_u8((data_size >> 16) as u8);
        buffer.put_u8((data_size >> 8) as u8);
        buffer.put_u8(data_size as u8);

        // Timestamp (24-bit) + Timestamp extended (8-bit)
        let ts = tag.timestamp;
        buffer.put_u8((ts >> 16) as u8);
        buffer.put_u8((ts >> 8) as u8);
        buffer.put_u8(ts as u8);
        buffer.put_u8((ts >> 24) as u8);

        // Stream ID (always 0)
        buffer.put_u8(0);
        buffer.put_u8(0);
        buffer.put_u8(0);

        // Tag data
        buffer.put_slice(&tag.data);

        // Previous tag size
        let tag_size = 11 + data_size;
        buffer.put_u32(tag_size as u32);

        Ok(buffer.freeze())
    }

    /// 检查是否已发送头部
    pub fn has_sent_header(&self) -> bool {
        self.has_sent_header
    }

    /// 重置状态
    pub fn reset(&mut self) {
        self.has_sent_header = false;
    }
}

impl Default for FlvMuxer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flv_header() {
        let mut muxer = FlvMuxer::new();
        assert!(!muxer.has_sent_header());

        let header = muxer.generate_header();
        assert!(muxer.has_sent_header());

        // 验证 FLV 头部格式
        assert_eq!(header.len(), 13);
        assert_eq!(&header[0..3], b"FLV");
        assert_eq!(header[3], 1); // Version
        assert_eq!(header[4], 0x05); // Flags (audio + video)
    }

    #[test]
    fn test_mux_video_tag() {
        let muxer = FlvMuxer::new();
        
        let tag = FlvTag {
            tag_type: FlvTagType::Video,
            timestamp: 1000,
            data: Bytes::from(vec![0x17, 0x01, 0x00, 0x00, 0x00]),
        };

        let result = muxer.mux_tag(&tag).unwrap();
        
        // 验证标签格式
        assert_eq!(result[0], 9); // Video tag type
        
        // Data size (3 bytes)
        let data_size = ((result[1] as usize) << 16) 
                      | ((result[2] as usize) << 8) 
                      | (result[3] as usize);
        assert_eq!(data_size, 5);

        // Timestamp (3 bytes + 1 byte extended)
        let ts = ((result[4] as u32) << 16) 
               | ((result[5] as u32) << 8) 
               | (result[6] as u32)
               | ((result[7] as u32) << 24);
        assert_eq!(ts, 1000);
    }

    #[test]
    fn test_mux_audio_tag() {
        let muxer = FlvMuxer::new();
        
        let tag = FlvTag {
            tag_type: FlvTagType::Audio,
            timestamp: 500,
            data: Bytes::from(vec![0xAF, 0x01]),
        };

        let result = muxer.mux_tag(&tag).unwrap();
        
        assert_eq!(result[0], 8); // Audio tag type
        
        let data_size = ((result[1] as usize) << 16) 
                      | ((result[2] as usize) << 8) 
                      | (result[3] as usize);
        assert_eq!(data_size, 2);
    }

    #[test]
    fn test_reset() {
        let mut muxer = FlvMuxer::new();
        muxer.generate_header();
        assert!(muxer.has_sent_header());

        muxer.reset();
        assert!(!muxer.has_sent_header());
    }
}
