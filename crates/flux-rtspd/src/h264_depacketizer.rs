use anyhow::Result;
use bytes::{Buf, Bytes, BytesMut};
use std::collections::VecDeque;
use tracing::{debug, warn};

use crate::rtp_receiver::RtpPacket;

/// H264 解包器
pub struct H264Depacketizer {
    buffer: VecDeque<RtpPacket>,
    last_timestamp: u32,
}

/// H264 NALU
#[derive(Debug, Clone)]
pub struct H264Nalu {
    pub timestamp: u32,
    pub data: Bytes,
    pub is_keyframe: bool,
}

impl H264Depacketizer {
    pub fn new() -> Self {
        Self {
            buffer: VecDeque::new(),
            last_timestamp: 0,
        }
    }

    /// 处理 RTP 包
    pub fn process_rtp(&mut self, packet: RtpPacket) -> Result<Vec<H264Nalu>> {
        let mut nalus = Vec::new();
        
        if packet.payload.is_empty() {
            return Ok(nalus);
        }
        
        let nal_type = packet.payload[0] & 0x1F;
        
        match nal_type {
            1..=23 => {
                // 单个 NALU
                nalus.push(self.create_nalu(packet.timestamp, packet.payload)?);
            }
            24 => {
                // STAP-A (多个 NALU 聚合)
                nalus.extend(self.process_stap_a(packet.timestamp, &packet.payload)?);
            }
            28 => {
                // FU-A (分片 NALU)
                if let Some(nalu) = self.process_fu_a(&packet)? {
                    nalus.push(nalu);
                }
            }
            29 => {
                // FU-B (分片 NALU with DON)
                warn!(target: "h264_depacketizer", "FU-B not implemented");
            }
            _ => {
                warn!(target: "h264_depacketizer", "Unknown NAL type: {}", nal_type);
            }
        }
        
        Ok(nalus)
    }

    /// 创建 NALU
    fn create_nalu(&self, timestamp: u32, data: Bytes) -> Result<H264Nalu> {
        let nal_type = data[0] & 0x1F;
        let is_keyframe = nal_type == 5; // IDR frame
        
        Ok(H264Nalu {
            timestamp,
            data,
            is_keyframe,
        })
    }

    /// 处理 STAP-A (Single-Time Aggregation Packet)
    fn process_stap_a(&self, timestamp: u32, payload: &[u8]) -> Result<Vec<H264Nalu>> {
        let mut nalus = Vec::new();
        let mut cursor = std::io::Cursor::new(&payload[1..]); // Skip STAP-A header
        
        while cursor.remaining() >= 2 {
            let nalu_size = cursor.get_u16() as usize;
            
            if cursor.remaining() < nalu_size {
                break;
            }
            
            let start = cursor.position() as usize;
            let end = start + nalu_size;
            let nalu_data = Bytes::copy_from_slice(&payload[1 + start..1 + end]);
            
            nalus.push(self.create_nalu(timestamp, nalu_data)?);
            cursor.set_position((start + nalu_size) as u64);
        }
        
        Ok(nalus)
    }

    /// 处理 FU-A (Fragmentation Unit)
    fn process_fu_a(&mut self, packet: &RtpPacket) -> Result<Option<H264Nalu>> {
        if packet.payload.len() < 2 {
            return Ok(None);
        }
        
        let fu_indicator = packet.payload[0];
        let fu_header = packet.payload[1];
        
        let start_bit = (fu_header & 0x80) != 0;
        let end_bit = (fu_header & 0x40) != 0;
        let nal_type = fu_header & 0x1F;
        
        if start_bit {
            // 开始新的分片序列
            self.buffer.clear();
            self.last_timestamp = packet.timestamp;
        }
        
        // 检查时间戳是否匹配
        if packet.timestamp != self.last_timestamp {
            warn!(target: "h264_depacketizer", "Timestamp mismatch in FU-A");
            self.buffer.clear();
            return Ok(None);
        }
        
        // 添加到缓冲区
        self.buffer.push_back(packet.clone());
        
        if end_bit {
            // 组装完整的 NALU
            let nalu = self.assemble_fu_a(nal_type, fu_indicator)?;
            self.buffer.clear();
            return Ok(Some(nalu));
        }
        
        Ok(None)
    }

    /// 组装 FU-A 分片
    fn assemble_fu_a(&self, nal_type: u8, fu_indicator: u8) -> Result<H264Nalu> {
        let mut data = BytesMut::new();
        
        // 重建 NAL header
        let nal_header = (fu_indicator & 0xE0) | nal_type;
        data.extend_from_slice(&[nal_header]);
        
        // 组装所有分片的 payload
        for packet in &self.buffer {
            if packet.payload.len() > 2 {
                data.extend_from_slice(&packet.payload[2..]); // Skip FU indicator and header
            }
        }
        
        let is_keyframe = nal_type == 5; // IDR frame
        
        Ok(H264Nalu {
            timestamp: self.last_timestamp,
            data: data.freeze(),
            is_keyframe,
        })
    }
}

impl Default for H264Depacketizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_nalu() {
        let mut depacketizer = H264Depacketizer::new();
        
        // 构造单个 NALU (NAL type 5 = IDR)
        let payload = Bytes::from(vec![0x65, 0x01, 0x02, 0x03]); // 0x65 = type 5
        let packet = RtpPacket {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: true,
            payload_type: 96,
            sequence_number: 1,
            timestamp: 1000,
            ssrc: 0x12345678,
            payload,
        };
        
        let nalus = depacketizer.process_rtp(packet).unwrap();
        assert_eq!(nalus.len(), 1);
        assert!(nalus[0].is_keyframe);
        assert_eq!(nalus[0].timestamp, 1000);
    }

    #[test]
    fn test_fu_a_fragmentation() {
        let mut depacketizer = H264Depacketizer::new();
        
        // FU-A 开始包
        let payload1 = Bytes::from(vec![
            0x7C, // FU indicator (type 28)
            0x85, // FU header: S=1, E=0, Type=5
            0x01, 0x02, 0x03,
        ]);
        let packet1 = RtpPacket {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: false,
            payload_type: 96,
            sequence_number: 1,
            timestamp: 2000,
            ssrc: 0x12345678,
            payload: payload1,
        };
        
        let nalus1 = depacketizer.process_rtp(packet1).unwrap();
        assert_eq!(nalus1.len(), 0); // 未完成
        
        // FU-A 结束包
        let payload2 = Bytes::from(vec![
            0x7C, // FU indicator
            0x45, // FU header: S=0, E=1, Type=5
            0x04, 0x05, 0x06,
        ]);
        let packet2 = RtpPacket {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: true,
            payload_type: 96,
            sequence_number: 2,
            timestamp: 2000,
            ssrc: 0x12345678,
            payload: payload2,
        };
        
        let nalus2 = depacketizer.process_rtp(packet2).unwrap();
        assert_eq!(nalus2.len(), 1);
        assert!(nalus2[0].is_keyframe);
    }
}
