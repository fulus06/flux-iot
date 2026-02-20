use anyhow::Result;
use bytes::{Buf, Bytes, BytesMut};
use std::collections::VecDeque;
use tracing::{debug, warn};

use crate::rtp_receiver::RtpPacket;

/// H265 解包器
pub struct H265Depacketizer {
    buffer: VecDeque<RtpPacket>,
    last_timestamp: u32,
}

/// H265 NALU
#[derive(Debug, Clone)]
pub struct H265Nalu {
    pub timestamp: u32,
    pub data: Bytes,
    pub is_keyframe: bool,
}

impl H265Depacketizer {
    pub fn new() -> Self {
        Self {
            buffer: VecDeque::new(),
            last_timestamp: 0,
        }
    }

    /// 处理 RTP 包
    pub fn process_rtp(&mut self, packet: RtpPacket) -> Result<Vec<H265Nalu>> {
        let mut nalus = Vec::new();
        
        if packet.payload.is_empty() {
            return Ok(nalus);
        }
        
        // H265 Payload Header (2 bytes)
        // +---------------+---------------+
        // |0|1|2|3|4|5|6|7|0|1|2|3|4|5|6|7|
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |F|   Type    |  LayerId  | TID |
        // +-------------+-----------------+
        
        let payload_header = u16::from_be_bytes([packet.payload[0], packet.payload[1]]);
        let nal_type = ((payload_header >> 9) & 0x3F) as u8;
        
        match nal_type {
            0..=47 => {
                // 单个 NALU
                nalus.push(self.create_nalu(packet.timestamp, packet.payload)?);
            }
            48 => {
                // AP (Aggregation Packet)
                nalus.extend(self.process_ap(packet.timestamp, &packet.payload)?);
            }
            49 => {
                // FU (Fragmentation Unit)
                if let Some(nalu) = self.process_fu(&packet)? {
                    nalus.push(nalu);
                }
            }
            50 => {
                // PACI (not commonly used)
                warn!(target: "h265_depacketizer", "PACI packet not implemented");
            }
            _ => {
                warn!(target: "h265_depacketizer", "Unknown NAL type: {}", nal_type);
            }
        }
        
        Ok(nalus)
    }

    /// 创建 NALU
    fn create_nalu(&self, timestamp: u32, data: Bytes) -> Result<H265Nalu> {
        let payload_header = u16::from_be_bytes([data[0], data[1]]);
        let nal_type = ((payload_header >> 9) & 0x3F) as u8;
        
        // H265 关键帧类型
        // 19-21: IDR (Instantaneous Decoding Refresh)
        // 32: VPS (Video Parameter Set)
        // 33: SPS (Sequence Parameter Set)
        // 34: PPS (Picture Parameter Set)
        let is_keyframe = matches!(nal_type, 19..=21 | 32..=34);
        
        Ok(H265Nalu {
            timestamp,
            data,
            is_keyframe,
        })
    }

    /// 处理 AP (Aggregation Packet)
    fn process_ap(&self, timestamp: u32, payload: &[u8]) -> Result<Vec<H265Nalu>> {
        let mut nalus = Vec::new();
        let mut cursor = std::io::Cursor::new(&payload[2..]); // Skip payload header
        
        while cursor.remaining() >= 2 {
            let nalu_size = cursor.get_u16() as usize;
            
            if cursor.remaining() < nalu_size {
                break;
            }
            
            let start = cursor.position() as usize;
            let end = start + nalu_size;
            let nalu_data = Bytes::copy_from_slice(&payload[2 + start..2 + end]);
            
            nalus.push(self.create_nalu(timestamp, nalu_data)?);
            cursor.set_position((start + nalu_size) as u64);
        }
        
        Ok(nalus)
    }

    /// 处理 FU (Fragmentation Unit)
    fn process_fu(&mut self, packet: &RtpPacket) -> Result<Option<H265Nalu>> {
        if packet.payload.len() < 3 {
            return Ok(None);
        }
        
        // FU Header (1 byte)
        // +---------------+
        // |0|1|2|3|4|5|6|7|
        // +-+-+-+-+-+-+-+-+
        // |S|E|  FuType   |
        // +---------------+
        
        let fu_header = packet.payload[2];
        let start_bit = (fu_header & 0x80) != 0;
        let end_bit = (fu_header & 0x40) != 0;
        let fu_type = fu_header & 0x3F;
        
        if start_bit {
            // 开始新的分片序列
            self.buffer.clear();
            self.last_timestamp = packet.timestamp;
        }
        
        // 检查时间戳是否匹配
        if packet.timestamp != self.last_timestamp {
            warn!(target: "h265_depacketizer", "Timestamp mismatch in FU");
            self.buffer.clear();
            return Ok(None);
        }
        
        // 添加到缓冲区
        self.buffer.push_back(packet.clone());
        
        if end_bit {
            // 组装完整的 NALU
            let nalu = self.assemble_fu(fu_type)?;
            self.buffer.clear();
            return Ok(Some(nalu));
        }
        
        Ok(None)
    }

    /// 组装 FU 分片
    fn assemble_fu(&self, fu_type: u8) -> Result<H265Nalu> {
        let mut data = BytesMut::new();
        
        if self.buffer.is_empty() {
            return Err(anyhow::anyhow!("Empty FU buffer"));
        }
        
        // 重建 Payload Header (从第一个包)
        let first_packet = &self.buffer[0];
        let payload_header = u16::from_be_bytes([first_packet.payload[0], first_packet.payload[1]]);
        
        // 替换 Type 字段为 FuType
        let new_payload_header = (payload_header & 0xFE00) | ((fu_type as u16) << 9);
        data.extend_from_slice(&new_payload_header.to_be_bytes());
        
        // 组装所有分片的 payload
        for packet in &self.buffer {
            if packet.payload.len() > 3 {
                data.extend_from_slice(&packet.payload[3..]); // Skip payload header + FU header
            }
        }
        
        let is_keyframe = matches!(fu_type, 19..=21 | 32..=34);
        
        Ok(H265Nalu {
            timestamp: self.last_timestamp,
            data: data.freeze(),
            is_keyframe,
        })
    }
}

impl Default for H265Depacketizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_nalu() {
        let mut depacketizer = H265Depacketizer::new();
        
        // 构造单个 NALU (NAL type 19 = IDR)
        let payload = Bytes::from(vec![
            0x26, 0x01, // Payload header: Type=19 (IDR)
            0x01, 0x02, 0x03,
        ]);
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
    fn test_fu_fragmentation() {
        let mut depacketizer = H265Depacketizer::new();
        
        // FU 开始包
        let payload1 = Bytes::from(vec![
            0x62, 0x01, // Payload header: Type=49 (FU)
            0x93,       // FU header: S=1, E=0, Type=19
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
        
        // FU 结束包
        let payload2 = Bytes::from(vec![
            0x62, 0x01, // Payload header
            0x53,       // FU header: S=0, E=1, Type=19
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
