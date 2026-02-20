use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io::Cursor;

/// SRT 握手类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeType {
    Induction = 1,      // 第一次握手（客户端 -> 服务器）
    Conclusion = -1,    // 第三次握手（客户端 -> 服务器）
    Agreement = -2,     // 第二次/第四次握手（服务器 -> 客户端）
}

impl HandshakeType {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Induction),
            -1 => Some(Self::Conclusion),
            -2 => Some(Self::Agreement),
            _ => None,
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::Induction => 1,
            Self::Conclusion => -1,
            Self::Agreement => -2,
        }
    }
}

/// SRT 版本
pub const SRT_VERSION: u32 = 0x00010400; // 1.4.0

/// SRT 握手包
#[derive(Debug, Clone)]
pub struct HandshakePacket {
    pub version: u32,
    pub encryption_field: u16,
    pub extension_field: u16,
    pub initial_packet_sequence: u32,
    pub mtu: u32,
    pub max_flow_window_size: u32,
    pub handshake_type: HandshakeType,
    pub srt_socket_id: u32,
    pub syn_cookie: u32,
    pub peer_ip_address: [u8; 16],
    // SRT 扩展字段
    pub extensions: Vec<u8>,
}

impl Default for HandshakePacket {
    fn default() -> Self {
        Self {
            version: SRT_VERSION,
            encryption_field: 0,
            extension_field: 0,
            initial_packet_sequence: 0,
            mtu: 1500,
            max_flow_window_size: 8192,
            handshake_type: HandshakeType::Induction,
            srt_socket_id: 0,
            syn_cookie: 0,
            peer_ip_address: [0; 16],
            extensions: Vec::new(),
        }
    }
}

impl HandshakePacket {
    /// 解析握手包
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < 48 {
            return Err("Handshake packet too short".to_string());
        }

        let mut cursor = Cursor::new(data);

        let version = cursor.get_u32();
        let encryption_field = cursor.get_u16();
        let extension_field = cursor.get_u16();
        let initial_packet_sequence = cursor.get_u32();
        let mtu = cursor.get_u32();
        let max_flow_window_size = cursor.get_u32();
        let handshake_type_value = cursor.get_i32();
        let srt_socket_id = cursor.get_u32();
        let syn_cookie = cursor.get_u32();

        let handshake_type = HandshakeType::from_i32(handshake_type_value)
            .ok_or_else(|| format!("Invalid handshake type: {}", handshake_type_value))?;

        let mut peer_ip_address = [0u8; 16];
        cursor.copy_to_slice(&mut peer_ip_address);

        let extensions = if data.len() > 48 {
            data[48..].to_vec()
        } else {
            Vec::new()
        };

        Ok(Self {
            version,
            encryption_field,
            extension_field,
            initial_packet_sequence,
            mtu,
            max_flow_window_size,
            handshake_type,
            srt_socket_id,
            syn_cookie,
            peer_ip_address,
            extensions,
        })
    }

    /// 序列化握手包
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(48 + self.extensions.len());

        buf.put_u32(self.version);
        buf.put_u16(self.encryption_field);
        buf.put_u16(self.extension_field);
        buf.put_u32(self.initial_packet_sequence);
        buf.put_u32(self.mtu);
        buf.put_u32(self.max_flow_window_size);
        buf.put_i32(self.handshake_type.to_i32());
        buf.put_u32(self.srt_socket_id);
        buf.put_u32(self.syn_cookie);
        buf.put_slice(&self.peer_ip_address);

        if !self.extensions.is_empty() {
            buf.put_slice(&self.extensions);
        }

        buf.freeze()
    }

    /// 创建 Induction 请求（第一次握手）
    pub fn create_induction_request(socket_id: u32) -> Self {
        Self {
            version: SRT_VERSION,
            handshake_type: HandshakeType::Induction,
            srt_socket_id: socket_id,
            ..Default::default()
        }
    }

    /// 创建 Induction 响应（第二次握手）
    pub fn create_induction_response(
        request: &HandshakePacket,
        server_socket_id: u32,
        syn_cookie: u32,
    ) -> Self {
        Self {
            version: SRT_VERSION,
            handshake_type: HandshakeType::Agreement,
            srt_socket_id: server_socket_id,
            syn_cookie,
            mtu: request.mtu,
            max_flow_window_size: request.max_flow_window_size,
            ..Default::default()
        }
    }

    /// 创建 Conclusion 请求（第三次握手）
    pub fn create_conclusion_request(
        socket_id: u32,
        syn_cookie: u32,
        initial_seq: u32,
    ) -> Self {
        Self {
            version: SRT_VERSION,
            handshake_type: HandshakeType::Conclusion,
            srt_socket_id: socket_id,
            syn_cookie,
            initial_packet_sequence: initial_seq,
            ..Default::default()
        }
    }

    /// 创建 Conclusion 响应（第四次握手）
    pub fn create_conclusion_response(
        request: &HandshakePacket,
        server_socket_id: u32,
    ) -> Self {
        Self {
            version: SRT_VERSION,
            handshake_type: HandshakeType::Agreement,
            srt_socket_id: server_socket_id,
            initial_packet_sequence: request.initial_packet_sequence,
            mtu: request.mtu,
            max_flow_window_size: request.max_flow_window_size,
            ..Default::default()
        }
    }
}

/// 握手状态机
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    Idle,
    InductionSent,
    InductionReceived,
    ConclusionSent,
    Connected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_type_conversion() {
        assert_eq!(HandshakeType::Induction.to_i32(), 1);
        assert_eq!(HandshakeType::Conclusion.to_i32(), -1);
        assert_eq!(HandshakeType::Agreement.to_i32(), -2);

        assert_eq!(HandshakeType::from_i32(1), Some(HandshakeType::Induction));
        assert_eq!(HandshakeType::from_i32(-1), Some(HandshakeType::Conclusion));
        assert_eq!(HandshakeType::from_i32(-2), Some(HandshakeType::Agreement));
    }

    #[test]
    fn test_serialize_parse_handshake() {
        let packet = HandshakePacket::create_induction_request(0x12345678);
        let serialized = packet.serialize();
        let parsed = HandshakePacket::parse(&serialized).unwrap();

        assert_eq!(parsed.version, SRT_VERSION);
        assert_eq!(parsed.handshake_type, HandshakeType::Induction);
        assert_eq!(parsed.srt_socket_id, 0x12345678);
    }

    #[test]
    fn test_handshake_flow() {
        // 第一次握手：客户端 -> 服务器
        let induction_req = HandshakePacket::create_induction_request(0x11111111);
        assert_eq!(induction_req.handshake_type, HandshakeType::Induction);

        // 第二次握手：服务器 -> 客户端
        let induction_resp = HandshakePacket::create_induction_response(
            &induction_req,
            0x22222222,
            0x12345678,
        );
        assert_eq!(induction_resp.handshake_type, HandshakeType::Agreement);
        assert_eq!(induction_resp.syn_cookie, 0x12345678);

        // 第三次握手：客户端 -> 服务器
        let conclusion_req = HandshakePacket::create_conclusion_request(
            0x11111111,
            0x12345678,
            1000,
        );
        assert_eq!(conclusion_req.handshake_type, HandshakeType::Conclusion);
        assert_eq!(conclusion_req.syn_cookie, 0x12345678);

        // 第四次握手：服务器 -> 客户端
        let conclusion_resp = HandshakePacket::create_conclusion_response(
            &conclusion_req,
            0x22222222,
        );
        assert_eq!(conclusion_resp.handshake_type, HandshakeType::Agreement);
    }
}
