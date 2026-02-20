use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io::Cursor;

/// SRT 包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Data,
    Control,
}

/// SRT 控制包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlType {
    Handshake = 0x0000,
    KeepAlive = 0x0001,
    Ack = 0x0002,
    Nak = 0x0003,
    CongestionWarning = 0x0004,
    Shutdown = 0x0005,
    AckAck = 0x0006,
    DropReq = 0x0007,
    PeerError = 0x0008,
    UserDefined = 0x7FFF,
}

impl ControlType {
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0000 => Some(Self::Handshake),
            0x0001 => Some(Self::KeepAlive),
            0x0002 => Some(Self::Ack),
            0x0003 => Some(Self::Nak),
            0x0004 => Some(Self::CongestionWarning),
            0x0005 => Some(Self::Shutdown),
            0x0006 => Some(Self::AckAck),
            0x0007 => Some(Self::DropReq),
            0x0008 => Some(Self::PeerError),
            0x7FFF => Some(Self::UserDefined),
            _ => None,
        }
    }
}

/// SRT 包头
#[derive(Debug, Clone)]
pub struct SrtHeader {
    pub is_control: bool,
    pub packet_seq_number: u32,
    pub timestamp: u32,
    pub dest_socket_id: u32,
}

/// SRT 数据包
#[derive(Debug, Clone)]
pub struct SrtDataPacket {
    pub header: SrtHeader,
    pub payload: Bytes,
}

/// SRT 控制包
#[derive(Debug, Clone)]
pub struct SrtControlPacket {
    pub header: SrtHeader,
    pub control_type: ControlType,
    pub type_specific_info: u32,
    pub payload: Bytes,
}

impl SrtHeader {
    /// 解析 SRT 包头
    pub fn parse(data: &[u8]) -> Result<(Self, usize), String> {
        if data.len() < 16 {
            return Err("Packet too short".to_string());
        }

        let mut cursor = Cursor::new(data);

        // Byte 0-3: Flags + Packet Sequence Number
        let word0 = cursor.get_u32();
        let is_control = (word0 & 0x80000000) != 0;
        let packet_seq_number = word0 & 0x7FFFFFFF;

        // Byte 4-7: Timestamp
        let timestamp = cursor.get_u32();

        // Byte 8-11: Destination Socket ID
        let dest_socket_id = cursor.get_u32();
        
        // Byte 12-15: Reserved (skip)
        let _ = cursor.get_u32();

        Ok((
            Self {
                is_control,
                packet_seq_number,
                timestamp,
                dest_socket_id,
            },
            16,
        ))
    }

    /// 序列化 SRT 包头
    pub fn serialize(&self, buf: &mut BytesMut) {
        let mut word0 = self.packet_seq_number & 0x7FFFFFFF;
        if self.is_control {
            word0 |= 0x80000000;
        }

        buf.put_u32(word0);
        buf.put_u32(self.timestamp);
        buf.put_u32(self.dest_socket_id);
        buf.put_u32(0); // Reserved
    }
}

impl SrtDataPacket {
    /// 解析数据包
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        let (header, offset) = SrtHeader::parse(data)?;

        if header.is_control {
            return Err("Not a data packet".to_string());
        }

        let payload = if data.len() > offset {
            Bytes::copy_from_slice(&data[offset..])
        } else {
            Bytes::new()
        };

        Ok(Self { header, payload })
    }

    /// 序列化数据包
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(16 + self.payload.len());
        self.header.serialize(&mut buf);
        buf.put_slice(&self.payload);
        buf.freeze()
    }
}

impl SrtControlPacket {
    /// 解析控制包
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        let (header, offset) = SrtHeader::parse(data)?;

        if !header.is_control {
            return Err("Not a control packet".to_string());
        }

        if data.len() < offset + 4 {
            return Err("Control packet too short".to_string());
        }

        let mut cursor = Cursor::new(&data[offset..]);

        // Control Type (16 bits) + Reserved (16 bits)
        let control_word = cursor.get_u32();
        let control_type_value = (control_word >> 16) as u16;
        let control_type = ControlType::from_u16(control_type_value)
            .ok_or_else(|| format!("Unknown control type: {}", control_type_value))?;

        // Type-specific information (only if enough data)
        let type_specific_info = if cursor.remaining() >= 4 {
            cursor.get_u32()
        } else {
            0
        };

        let payload_offset = offset + 4 + if type_specific_info != 0 || cursor.position() > 4 { 4 } else { 0 };

        let payload = if data.len() > payload_offset {
            Bytes::copy_from_slice(&data[payload_offset..])
        } else {
            Bytes::new()
        };

        Ok(Self {
            header,
            control_type,
            type_specific_info,
            payload,
        })
    }

    /// 序列化控制包
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(24 + self.payload.len());
        self.header.serialize(&mut buf);

        // Control Type + Reserved
        let control_word = ((self.control_type as u32) << 16) | 0x0000;
        buf.put_u32(control_word);

        // Type-specific information
        buf.put_u32(self.type_specific_info);

        buf.put_slice(&self.payload);
        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_data_packet() {
        let data = vec![
            0x00, 0x00, 0x00, 0x01, // Seq = 1, Data packet
            0x00, 0x00, 0x00, 0x64, // Timestamp = 100
            0x12, 0x34, 0x56, 0x78, // Dest Socket ID
            0x00, 0x00, 0x00, 0x00, // Reserved (4 bytes to complete 16-byte header)
            0x48, 0x65, 0x6C, 0x6C, 0x6F, // "Hello"
        ];

        let packet = SrtDataPacket::parse(&data).unwrap();
        assert!(!packet.header.is_control);
        assert_eq!(packet.header.packet_seq_number, 1);
        assert_eq!(packet.header.timestamp, 100);
        assert_eq!(packet.payload.as_ref(), b"Hello");
    }

    #[test]
    fn test_parse_control_packet() {
        let data = vec![
            0x80, 0x00, 0x00, 0x00, // Control packet
            0x00, 0x00, 0x00, 0xC8, // Timestamp = 200
            0xAB, 0xCD, 0xEF, 0x00, // Dest Socket ID
            0x00, 0x00, 0x00, 0x00, // Reserved (complete 16-byte header)
            0x00, 0x01, 0x00, 0x00, // Control Type = KeepAlive
            0x00, 0x00, 0x00, 0x00, // Type-specific info
        ];

        let packet = SrtControlPacket::parse(&data).unwrap();
        assert!(packet.header.is_control);
        assert_eq!(packet.control_type, ControlType::KeepAlive);
    }

    #[test]
    fn test_serialize_data_packet() {
        let packet = SrtDataPacket {
            header: SrtHeader {
                is_control: false,
                packet_seq_number: 42,
                timestamp: 1000,
                dest_socket_id: 0x12345678,
            },
            payload: Bytes::from("test"),
        };

        let serialized = packet.serialize();
        let parsed = SrtDataPacket::parse(&serialized).unwrap();

        assert_eq!(parsed.header.packet_seq_number, 42);
        assert_eq!(parsed.header.timestamp, 1000);
        assert_eq!(parsed.payload.as_ref(), b"test");
    }
}
