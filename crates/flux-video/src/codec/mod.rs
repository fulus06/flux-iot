// 编解码模块（轻量级，仅 NALU 解析）
use bytes::Bytes;

/// H.264 NALU 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264NaluType {
    Slice = 1,
    Idr = 5,
    Sei = 6,
    Sps = 7,
    Pps = 8,
    Unknown,
}

impl H264NaluType {
    /// 是否为关键帧（IDR）
    pub fn is_keyframe(&self) -> bool {
        matches!(self, Self::Idr)
    }
    
    /// 是否为参数集（SPS/PPS）
    pub fn is_parameter_set(&self) -> bool {
        matches!(self, Self::Sps | Self::Pps)
    }
}

impl From<u8> for H264NaluType {
    fn from(value: u8) -> Self {
        match value & 0x1F {
            1 => Self::Slice,
            5 => Self::Idr,
            6 => Self::Sei,
            7 => Self::Sps,
            8 => Self::Pps,
            _ => Self::Unknown,
        }
    }
}

/// H.264 NALU（网络抽象层单元）
#[derive(Debug, Clone)]
pub struct H264Nalu {
    pub nalu_type: H264NaluType,
    pub data: Bytes,
}

impl H264Nalu {
    /// 从数据中解析 NALU
    pub fn parse(data: Bytes) -> Option<Self> {
        if data.is_empty() {
            return None;
        }
        
        // 第一个字节包含 NALU 类型
        let nalu_type = H264NaluType::from(data[0]);
        
        Some(Self { nalu_type, data })
    }
    
    /// 是否为关键帧
    pub fn is_keyframe(&self) -> bool {
        self.nalu_type.is_keyframe()
    }
}

/// H.264 帧解析器
pub struct H264Parser {
    /// SPS（序列参数集）
    sps: Option<Bytes>,
    
    /// PPS（图像参数集）
    pps: Option<Bytes>,
}

impl H264Parser {
    pub fn new() -> Self {
        Self {
            sps: None,
            pps: None,
        }
    }
    
    /// 解析 Annex B 格式的数据（带起始码）
    pub fn parse_annexb(&mut self, data: &[u8]) -> Vec<H264Nalu> {
        let mut nalus = Vec::new();
        let mut pos = 0;
        
        while pos < data.len() {
            // 查找起始码
            if let Some((start_code_end, nalu_start)) = self.find_start_code_with_pos(data, pos) {
                // 查找下一个起始码
                let nalu_end = if let Some((next_start, _)) = self.find_start_code_with_pos(data, start_code_end) {
                    next_start
                } else {
                    data.len()
                };
                
                // 提取 NALU 数据（不包含起始码）
                if nalu_start < nalu_end {
                    let nalu_data = Bytes::copy_from_slice(&data[nalu_start..nalu_end]);
                    
                    if let Some(nalu) = H264Nalu::parse(nalu_data) {
                        // 缓存 SPS/PPS
                        match nalu.nalu_type {
                            H264NaluType::Sps => {
                                self.sps = Some(nalu.data.clone());
                            }
                            H264NaluType::Pps => {
                                self.pps = Some(nalu.data.clone());
                            }
                            _ => {}
                        }
                        
                        nalus.push(nalu);
                    }
                }
                
                pos = start_code_end;
            } else {
                break;
            }
        }
        
        nalus
    }
    
    /// 查找起始码位置，返回 (起始码开始位置, NALU 数据开始位置)
    fn find_start_code_with_pos(&self, data: &[u8], start: usize) -> Option<(usize, usize)> {
        if start + 3 > data.len() {
            return None;
        }
        
        for i in start..data.len() - 3 {
            // 0x00 0x00 0x00 0x01
            if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 0 && data[i + 3] == 1 {
                return Some((i + 4, i + 4));
            }
            // 0x00 0x00 0x01
            if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
                return Some((i + 3, i + 3));
            }
        }
        
        None
    }
    
    /// 获取 SPS
    pub fn sps(&self) -> Option<&Bytes> {
        self.sps.as_ref()
    }
    
    /// 获取 PPS
    pub fn pps(&self) -> Option<&Bytes> {
        self.pps.as_ref()
    }
    
    /// 是否有完整的参数集
    pub fn has_parameter_sets(&self) -> bool {
        self.sps.is_some() && self.pps.is_some()
    }
}

impl Default for H264Parser {
    fn default() -> Self {
        Self::new()
    }
}
