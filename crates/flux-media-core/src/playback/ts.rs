use crate::error::Result;
use bytes::{BufMut, Bytes, BytesMut};

/// MPEG-TS 封装器
pub struct TsMuxer {
    pat_pmt_sent: bool,
    video_pid: u16,
    audio_pid: u16,
    pcr_pid: u16,
    continuity_counter_pat: u8,
    continuity_counter_pmt: u8,
    continuity_counter_video: u8,
    continuity_counter_audio: u8,
}

impl TsMuxer {
    pub fn new() -> Self {
        Self {
            pat_pmt_sent: false,
            video_pid: 0x100,
            audio_pid: 0x101,
            pcr_pid: 0x100,
            continuity_counter_pat: 0,
            continuity_counter_pmt: 0,
            continuity_counter_video: 0,
            continuity_counter_audio: 0,
        }
    }

    /// 生成 PAT (Program Association Table)
    fn generate_pat(&mut self) -> Bytes {
        let mut packet = BytesMut::with_capacity(188);
        
        // TS Header
        packet.put_u8(0x47); // Sync byte
        packet.put_u8(0x40); // Payload unit start indicator
        packet.put_u8(0x00); // PID = 0 (PAT)
        packet.put_u8(0x10 | (self.continuity_counter_pat & 0x0F));
        self.continuity_counter_pat = (self.continuity_counter_pat + 1) & 0x0F;

        // Pointer field
        packet.put_u8(0x00);

        // PAT
        packet.put_u8(0x00); // Table ID
        packet.put_u8(0xB0); // Section syntax indicator
        packet.put_u8(0x0D); // Section length
        packet.put_u16(0x0001); // Transport stream ID
        packet.put_u8(0xC1); // Version 0, current
        packet.put_u8(0x00); // Section number
        packet.put_u8(0x00); // Last section number
        packet.put_u16(0x0001); // Program number
        packet.put_u16(0xE000 | 0x1000); // PMT PID = 0x1000

        // CRC32 (simplified - should calculate actual CRC)
        packet.put_u32(0x00000000);

        // Padding
        while packet.len() < 188 {
            packet.put_u8(0xFF);
        }

        packet.freeze()
    }

    /// 生成 PMT (Program Map Table)
    fn generate_pmt(&mut self) -> Bytes {
        let mut packet = BytesMut::with_capacity(188);
        
        // TS Header
        packet.put_u8(0x47); // Sync byte
        packet.put_u8(0x50); // Payload unit start indicator
        packet.put_u8(0x00); // PID = 0x1000 (PMT)
        packet.put_u8(0x10 | (self.continuity_counter_pmt & 0x0F));
        self.continuity_counter_pmt = (self.continuity_counter_pmt + 1) & 0x0F;

        // Pointer field
        packet.put_u8(0x00);

        // PMT
        packet.put_u8(0x02); // Table ID
        packet.put_u8(0xB0); // Section syntax indicator
        packet.put_u8(0x17); // Section length
        packet.put_u16(0x0001); // Program number
        packet.put_u8(0xC1); // Version 0, current
        packet.put_u8(0x00); // Section number
        packet.put_u8(0x00); // Last section number
        packet.put_u16(0xE000 | self.pcr_pid); // PCR PID

        // Program info length
        packet.put_u16(0xF000);

        // Video stream (H.264)
        packet.put_u8(0x1B); // Stream type (H.264)
        packet.put_u16(0xE000 | self.video_pid);
        packet.put_u16(0xF000); // ES info length

        // Audio stream (AAC)
        packet.put_u8(0x0F); // Stream type (AAC)
        packet.put_u16(0xE000 | self.audio_pid);
        packet.put_u16(0xF000); // ES info length

        // CRC32
        packet.put_u32(0x00000000);

        // Padding
        while packet.len() < 188 {
            packet.put_u8(0xFF);
        }

        packet.freeze()
    }

    /// 封装 PES 包
    pub fn mux_video_pes(&mut self, data: &[u8], pts: u64, dts: u64, is_keyframe: bool) -> Result<Vec<Bytes>> {
        let mut packets = Vec::new();

        // 如果还没发送 PAT/PMT，先发送
        if !self.pat_pmt_sent {
            packets.push(self.generate_pat());
            packets.push(self.generate_pmt());
            self.pat_pmt_sent = true;
        }

        // 构造 PES header
        let mut pes = BytesMut::new();
        pes.put_slice(&[0x00, 0x00, 0x01]); // Packet start code
        pes.put_u8(0xE0); // Stream ID (video)
        
        // PES packet length (0 = unbounded)
        pes.put_u16(0);

        // PES header flags
        pes.put_u8(0x80); // Marker bits
        pes.put_u8(0xC0); // PTS + DTS flags
        pes.put_u8(10); // PES header length

        // PTS
        pes.put_u8(0x31 | (((pts >> 30) & 0x07) << 1) as u8);
        pes.put_u16((((pts >> 15) & 0x7FFF) << 1) as u16);
        pes.put_u16((((pts) & 0x7FFF) << 1) as u16);

        // DTS
        pes.put_u8(0x11 | (((dts >> 30) & 0x07) << 1) as u8);
        pes.put_u16((((dts >> 15) & 0x7FFF) << 1) as u16);
        pes.put_u16((((dts) & 0x7FFF) << 1) as u16);

        // PES data
        pes.put_slice(data);

        // 分割成 TS 包
        let pes_data = pes.freeze();
        let mut offset = 0;
        let mut first_packet = true;

        while offset < pes_data.len() {
            let mut packet = BytesMut::with_capacity(188);
            
            // TS Header
            packet.put_u8(0x47); // Sync byte
            
            let mut flags = 0x00;
            if first_packet {
                flags |= 0x40; // Payload unit start indicator
                if is_keyframe {
                    flags |= 0x20; // Adaptation field present
                }
            }
            packet.put_u8(flags);
            packet.put_u8((self.video_pid >> 8) as u8);
            packet.put_u8((self.video_pid & 0xFF) as u8 | 0x10 | (self.continuity_counter_video & 0x0F));
            self.continuity_counter_video = (self.continuity_counter_video + 1) & 0x0F;

            // Adaptation field (for keyframe)
            if first_packet && is_keyframe {
                packet.put_u8(7); // Adaptation field length
                packet.put_u8(0x50); // Random access indicator
                for _ in 0..6 {
                    packet.put_u8(0xFF); // Stuffing
                }
            }

            // Payload
            let payload_size = 188 - packet.len();
            let chunk_size = std::cmp::min(payload_size, pes_data.len() - offset);
            packet.put_slice(&pes_data[offset..offset + chunk_size]);
            offset += chunk_size;

            // Padding
            while packet.len() < 188 {
                packet.put_u8(0xFF);
            }

            packets.push(packet.freeze());
            first_packet = false;
        }

        Ok(packets)
    }

    /// 重置状态
    pub fn reset(&mut self) {
        self.pat_pmt_sent = false;
        self.continuity_counter_pat = 0;
        self.continuity_counter_pmt = 0;
        self.continuity_counter_video = 0;
        self.continuity_counter_audio = 0;
    }
}

impl Default for TsMuxer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ts_muxer_creation() {
        let muxer = TsMuxer::new();
        assert!(!muxer.pat_pmt_sent);
        assert_eq!(muxer.video_pid, 0x100);
    }

    #[test]
    fn test_generate_pat() {
        let mut muxer = TsMuxer::new();
        let pat = muxer.generate_pat();
        
        assert_eq!(pat.len(), 188);
        assert_eq!(pat[0], 0x47); // Sync byte
    }

    #[test]
    fn test_generate_pmt() {
        let mut muxer = TsMuxer::new();
        let pmt = muxer.generate_pmt();
        
        assert_eq!(pmt.len(), 188);
        assert_eq!(pmt[0], 0x47); // Sync byte
    }

    #[test]
    fn test_mux_video_pes() {
        let mut muxer = TsMuxer::new();
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x67]; // H.264 SPS
        
        let packets = muxer.mux_video_pes(&data, 90000, 90000, true).unwrap();
        
        // 应该包含 PAT + PMT + 至少一个视频包
        assert!(packets.len() >= 3);
        assert!(muxer.pat_pmt_sent);
    }

    #[test]
    fn test_reset() {
        let mut muxer = TsMuxer::new();
        muxer.pat_pmt_sent = true;
        muxer.continuity_counter_video = 5;
        
        muxer.reset();
        
        assert!(!muxer.pat_pmt_sent);
        assert_eq!(muxer.continuity_counter_video, 0);
    }
}
