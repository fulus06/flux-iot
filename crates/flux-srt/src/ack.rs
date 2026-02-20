use bytes::{BufMut, Bytes, BytesMut};

/// ACK 包结构
#[derive(Debug, Clone)]
pub struct AckPacket {
    pub last_ack_seq: u32,
    pub rtt: u32,           // 往返时延（微秒）
    pub rtt_variance: u32,  // RTT 方差
    pub available_buffer: u32, // 可用缓冲区大小
    pub packet_recv_rate: u32, // 包接收速率
    pub estimated_link_capacity: u32, // 估计链路容量
}

impl AckPacket {
    pub fn new(last_ack_seq: u32) -> Self {
        Self {
            last_ack_seq,
            rtt: 0,
            rtt_variance: 0,
            available_buffer: 8192,
            packet_recv_rate: 0,
            estimated_link_capacity: 0,
        }
    }

    /// 序列化 ACK 包
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(24);

        buf.put_u32(self.last_ack_seq);
        buf.put_u32(self.rtt);
        buf.put_u32(self.rtt_variance);
        buf.put_u32(self.available_buffer);
        buf.put_u32(self.packet_recv_rate);
        buf.put_u32(self.estimated_link_capacity);

        buf.freeze()
    }

    /// 解析 ACK 包
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < 24 {
            return Err("ACK packet too short".to_string());
        }

        use bytes::Buf;
        let mut cursor = std::io::Cursor::new(data);

        Ok(Self {
            last_ack_seq: cursor.get_u32(),
            rtt: cursor.get_u32(),
            rtt_variance: cursor.get_u32(),
            available_buffer: cursor.get_u32(),
            packet_recv_rate: cursor.get_u32(),
            estimated_link_capacity: cursor.get_u32(),
        })
    }
}

/// NAK 包结构（丢包通知）
#[derive(Debug, Clone)]
pub struct NakPacket {
    pub lost_sequences: Vec<u32>,
}

impl NakPacket {
    pub fn new(lost_sequences: Vec<u32>) -> Self {
        Self { lost_sequences }
    }

    /// 序列化 NAK 包
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(4 + self.lost_sequences.len() * 4);

        // 丢失包数量
        buf.put_u32(self.lost_sequences.len() as u32);

        // 丢失的序列号列表
        for seq in &self.lost_sequences {
            buf.put_u32(*seq);
        }

        buf.freeze()
    }

    /// 解析 NAK 包
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < 4 {
            return Err("NAK packet too short".to_string());
        }

        use bytes::Buf;
        let mut cursor = std::io::Cursor::new(data);

        let count = cursor.get_u32() as usize;

        if data.len() < 4 + count * 4 {
            return Err("NAK packet incomplete".to_string());
        }

        let mut lost_sequences = Vec::with_capacity(count);
        for _ in 0..count {
            lost_sequences.push(cursor.get_u32());
        }

        Ok(Self { lost_sequences })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_serialize_parse() {
        let ack = AckPacket {
            last_ack_seq: 100,
            rtt: 50000,
            rtt_variance: 1000,
            available_buffer: 8192,
            packet_recv_rate: 1000,
            estimated_link_capacity: 100000,
        };

        let serialized = ack.serialize();
        let parsed = AckPacket::parse(&serialized).unwrap();

        assert_eq!(parsed.last_ack_seq, 100);
        assert_eq!(parsed.rtt, 50000);
        assert_eq!(parsed.available_buffer, 8192);
    }

    #[test]
    fn test_nak_serialize_parse() {
        let nak = NakPacket::new(vec![10, 15, 20, 25]);

        let serialized = nak.serialize();
        let parsed = NakPacket::parse(&serialized).unwrap();

        assert_eq!(parsed.lost_sequences.len(), 4);
        assert_eq!(parsed.lost_sequences, vec![10, 15, 20, 25]);
    }

    #[test]
    fn test_nak_empty() {
        let nak = NakPacket::new(vec![]);

        let serialized = nak.serialize();
        let parsed = NakPacket::parse(&serialized).unwrap();

        assert_eq!(parsed.lost_sequences.len(), 0);
    }
}
