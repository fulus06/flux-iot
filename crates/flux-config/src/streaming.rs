use serde::{Deserialize, Serialize};

/// 流媒体配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamingConfig {
    /// 转码配置
    #[serde(default)]
    pub transcode: TranscodeConfig,
    
    /// 输出协议配置
    #[serde(default)]
    pub outputs: Vec<OutputProtocol>,
}

/// 转码配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscodeConfig {
    /// 是否启用转码
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// 工作模式
    #[serde(default)]
    pub mode: TranscodeMode,
    
    /// 硬件加速类型
    #[serde(default)]
    pub hardware_accel: Option<HardwareAccel>,
    
    /// 目标码率配置
    #[serde(default)]
    pub bitrates: Vec<BitrateConfig>,
}

/// 转码模式
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranscodeMode {
    /// 直通模式（零转码）
    Passthrough {
        /// 是否需要重新封装
        #[serde(default = "default_true")]
        remux: bool,
    },
    
    /// 转码模式
    Transcode,
    
    /// 自动模式（根据触发条件决定）
    Auto {
        /// 触发条件
        #[serde(default)]
        triggers: Vec<TranscodeTrigger>,
    },
}

/// 转码触发条件
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranscodeTrigger {
    /// 协议切换触发
    /// 当客户端请求不同协议时自动转码
    ProtocolSwitch,
    
    /// 客户端类型多样性触发
    /// 当检测到不同类型客户端时转码
    ClientVariety,
    
    /// 网络质量差异触发
    /// 当检测到客户端网络质量差异时转码
    NetworkVariance {
        /// 带宽差异阈值（百分比，0.0-1.0）
        #[serde(default = "default_variance_threshold")]
        threshold: f64,
    },
    
    /// 客户端数量触发
    /// 当客户端数量超过阈值时转码
    ClientThreshold {
        /// 客户端数量阈值
        count: usize,
    },
    
    /// 永不转码
    Never,
}

/// 硬件加速类型
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HardwareAccel {
    /// NVIDIA GPU (NVENC)
    NVENC,
    
    /// Intel Quick Sync (QSV)
    QSV,
    
    /// Apple VideoToolbox
    VideoToolbox,
    
    /// Linux VAAPI
    VAAPI,
}

/// 码率配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BitrateConfig {
    /// 变体名称
    pub name: String,
    
    /// 码率 (kbps)
    pub bitrate: u32,
    
    /// 分辨率 (宽x高)
    pub resolution: (u32, u32),
    
    /// 帧率
    #[serde(default = "default_framerate")]
    pub framerate: f32,
    
    /// 编码器预设
    #[serde(default = "default_encoder_preset")]
    pub encoder_preset: String,
}

/// 输出协议
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputProtocol {
    HLS,
    FLV,
    RTMP,
    RTSP,
    WebRTC,
}

// 默认值函数
fn default_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_variance_threshold() -> f64 {
    0.5 // 50% 带宽差异
}

fn default_framerate() -> f32 {
    25.0
}

fn default_encoder_preset() -> String {
    "fast".to_string()
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            transcode: TranscodeConfig::default(),
            outputs: vec![OutputProtocol::HLS, OutputProtocol::FLV],
        }
    }
}

impl Default for TranscodeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: TranscodeMode::default(),
            hardware_accel: None,
            bitrates: vec![
                BitrateConfig {
                    name: "high".to_string(),
                    bitrate: 2000,
                    resolution: (1920, 1080),
                    framerate: 25.0,
                    encoder_preset: "fast".to_string(),
                },
                BitrateConfig {
                    name: "medium".to_string(),
                    bitrate: 1000,
                    resolution: (1280, 720),
                    framerate: 25.0,
                    encoder_preset: "fast".to_string(),
                },
                BitrateConfig {
                    name: "low".to_string(),
                    bitrate: 500,
                    resolution: (640, 360),
                    framerate: 25.0,
                    encoder_preset: "veryfast".to_string(),
                },
            ],
        }
    }
}

impl Default for TranscodeMode {
    fn default() -> Self {
        // 默认使用自动模式，协议切换时触发转码
        TranscodeMode::Auto {
            triggers: vec![TranscodeTrigger::ProtocolSwitch],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_streaming_config() {
        let config = StreamingConfig::default();
        assert!(matches!(config.transcode.mode, TranscodeMode::Auto { .. }));
        assert_eq!(config.outputs.len(), 2);
    }

    #[test]
    fn test_transcode_mode_passthrough() {
        let mode = TranscodeMode::Passthrough { remux: true };
        let json = serde_json::to_string(&mode).unwrap();
        assert!(json.contains("passthrough"));
    }

    #[test]
    fn test_transcode_trigger_protocol_switch() {
        let trigger = TranscodeTrigger::ProtocolSwitch;
        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("protocol_switch"));
    }

    #[test]
    fn test_transcode_trigger_client_threshold() {
        let trigger = TranscodeTrigger::ClientThreshold { count: 5 };
        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("client_threshold"));
        assert!(json.contains("\"count\":5"));
    }

    #[test]
    fn test_hardware_accel_nvenc() {
        let hw = HardwareAccel::NVENC;
        let json = serde_json::to_string(&hw).unwrap();
        assert_eq!(json, "\"nvenc\"");
    }

    #[test]
    fn test_bitrate_config() {
        let bitrate = BitrateConfig {
            name: "high".to_string(),
            bitrate: 2000,
            resolution: (1920, 1080),
            framerate: 25.0,
            encoder_preset: "fast".to_string(),
        };
        
        assert_eq!(bitrate.bitrate, 2000);
        assert_eq!(bitrate.resolution, (1920, 1080));
    }
}
