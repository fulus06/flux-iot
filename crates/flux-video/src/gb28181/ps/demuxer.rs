// PS 流解封装器
// 从 PS 流中提取视频/音频数据

use super::packet::{PsPacket, PsPacketType};
use bytes::{Bytes, BytesMut};
use std::collections::VecDeque;

/// PS 解封装器
pub struct PsDemuxer {
    /// 缓冲区
    buffer: BytesMut,
    
    /// 视频数据队列
    video_queue: VecDeque<Bytes>,
    
    /// 音频数据队列
    audio_queue: VecDeque<Bytes>,
}

impl PsDemuxer {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(65536),
            video_queue: VecDeque::new(),
            audio_queue: VecDeque::new(),
        }
    }
    
    /// 输入 PS 数据
    pub fn input(&mut self, data: Bytes) {
        self.buffer.extend_from_slice(&data);
        self.process_buffer();
    }
    
    /// 处理缓冲区
    fn process_buffer(&mut self) {
        while self.buffer.len() >= 4 {
            // 查找起始码
            if let Some(pos) = PsPacket::find_start_code(&self.buffer, 0) {
                if pos > 0 {
                    // 丢弃起始码之前的数据
                    let _ = self.buffer.split_to(pos);
                }
                
                // 检查是否有足够的数据
                if self.buffer.len() < 4 {
                    break;
                }
                
                // 读取流 ID
                let stream_id = self.buffer[3];
                let packet_type = PsPacketType::from_stream_id(stream_id);
                
                // 根据包类型处理
                match packet_type {
                    PsPacketType::PackHeader => {
                        if let Some(len) = self.parse_pack_header() {
                            let _ = self.buffer.split_to(len);
                        } else {
                            break;
                        }
                    }
                    PsPacketType::SystemHeader => {
                        if let Some(len) = self.parse_system_header() {
                            let _ = self.buffer.split_to(len);
                        } else {
                            break;
                        }
                    }
                    PsPacketType::ProgramStreamMap => {
                        if let Some(len) = self.parse_program_stream_map() {
                            let _ = self.buffer.split_to(len);
                        } else {
                            break;
                        }
                    }
                    PsPacketType::Video => {
                        if let Some(data) = self.parse_pes_packet() {
                            self.video_queue.push_back(data);
                        } else {
                            break;
                        }
                    }
                    PsPacketType::Audio => {
                        if let Some(data) = self.parse_pes_packet() {
                            self.audio_queue.push_back(data);
                        } else {
                            break;
                        }
                    }
                    PsPacketType::Unknown => {
                        // 跳过未知包
                        let _ = self.buffer.split_to(4);
                    }
                }
            } else {
                // 没有找到起始码，清空缓冲区
                self.buffer.clear();
                break;
            }
        }
    }
    
    /// 解析 Pack Header
    fn parse_pack_header(&mut self) -> Option<usize> {
        if self.buffer.len() < 14 {
            return None;
        }
        
        // Pack header 固定 14 字节
        let mut len = 14;
        
        // 检查是否有 stuffing bytes
        if self.buffer.len() > 13 {
            let stuffing_len = (self.buffer[13] & 0x07) as usize;
            len += stuffing_len;
        }
        
        if self.buffer.len() < len {
            return None;
        }
        
        Some(len)
    }
    
    /// 解析 System Header
    fn parse_system_header(&mut self) -> Option<usize> {
        if self.buffer.len() < 6 {
            return None;
        }
        
        // 读取长度
        let length = u16::from_be_bytes([self.buffer[4], self.buffer[5]]) as usize;
        let total_len = 6 + length;
        
        if self.buffer.len() < total_len {
            return None;
        }
        
        Some(total_len)
    }
    
    /// 解析 Program Stream Map
    fn parse_program_stream_map(&mut self) -> Option<usize> {
        if self.buffer.len() < 6 {
            return None;
        }
        
        // 读取长度
        let length = u16::from_be_bytes([self.buffer[4], self.buffer[5]]) as usize;
        let total_len = 6 + length;
        
        if self.buffer.len() < total_len {
            return None;
        }
        
        Some(total_len)
    }
    
    /// 解析 PES 包
    fn parse_pes_packet(&mut self) -> Option<Bytes> {
        if self.buffer.len() < 6 {
            return None;
        }
        
        // 读取 PES 包长度
        let pes_length = u16::from_be_bytes([self.buffer[4], self.buffer[5]]) as usize;
        
        if pes_length == 0 {
            // 长度为 0 表示不限长度，需要查找下一个起始码
            if let Some(next_pos) = PsPacket::find_start_code(&self.buffer, 6) {
                let data = self.buffer.split_to(next_pos).freeze();
                // 跳过 PES 头部，提取负载
                if data.len() > 9 {
                    let pes_header_len = data[8] as usize;
                    let payload_start = 9 + pes_header_len;
                    if data.len() > payload_start {
                        return Some(data.slice(payload_start..));
                    }
                }
                return None;
            } else {
                // 没有找到下一个起始码，等待更多数据
                return None;
            }
        }
        
        let total_len = 6 + pes_length;
        
        if self.buffer.len() < total_len {
            return None;
        }
        
        // 提取 PES 包
        let pes_data = self.buffer.split_to(total_len).freeze();
        
        // 跳过 PES 头部，提取负载
        if pes_data.len() > 9 {
            let pes_header_len = pes_data[8] as usize;
            let payload_start = 9 + pes_header_len;
            
            if pes_data.len() > payload_start {
                return Some(pes_data.slice(payload_start..));
            }
        }
        
        None
    }
    
    /// 获取视频数据
    pub fn pop_video(&mut self) -> Option<Bytes> {
        self.video_queue.pop_front()
    }
    
    /// 获取音频数据
    pub fn pop_audio(&mut self) -> Option<Bytes> {
        self.audio_queue.pop_front()
    }
    
    /// 获取视频队列长度
    pub fn video_queue_len(&self) -> usize {
        self.video_queue.len()
    }
    
    /// 获取音频队列长度
    pub fn audio_queue_len(&self) -> usize {
        self.audio_queue.len()
    }
    
    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.video_queue.clear();
        self.audio_queue.clear();
    }
}

impl Default for PsDemuxer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ps_demuxer_creation() {
        let demuxer = PsDemuxer::new();
        assert_eq!(demuxer.video_queue_len(), 0);
        assert_eq!(demuxer.audio_queue_len(), 0);
    }
    
    #[test]
    fn test_ps_demuxer_clear() {
        let mut demuxer = PsDemuxer::new();
        demuxer.input(Bytes::from(vec![0x00, 0x00, 0x01, 0xBA]));
        demuxer.clear();
        assert_eq!(demuxer.video_queue_len(), 0);
    }
}
