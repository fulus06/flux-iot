use flux_config::HardwareAccel;

/// FFmpeg 性能配置
#[derive(Debug, Clone)]
pub struct FfmpegConfig {
    /// 线程数（0 = 自动）
    pub threads: u32,
    
    /// 缓冲区大小（字节）
    pub buffer_size: usize,
    
    /// GOP 大小（关键帧间隔）
    pub gop_size: u32,
    
    /// B 帧数量
    pub b_frames: u32,
    
    /// 参考帧数量
    pub ref_frames: u32,
    
    /// 编码预设
    pub preset: Preset,
    
    /// 码率控制模式
    pub rate_control: RateControl,
    
    /// 是否启用低延迟
    pub low_latency: bool,
    
    /// 是否启用零拷贝
    pub zero_copy: bool,
}

/// 编码预设
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// 超快速（最低质量，最快速度）
    UltraFast,
    /// 超快
    SuperFast,
    /// 非常快
    VeryFast,
    /// 快速
    Fast,
    /// 中等
    Medium,
    /// 慢速
    Slow,
    /// 非常慢
    VerySlow,
}

impl Preset {
    pub fn as_str(&self) -> &str {
        match self {
            Preset::UltraFast => "ultrafast",
            Preset::SuperFast => "superfast",
            Preset::VeryFast => "veryfast",
            Preset::Fast => "fast",
            Preset::Medium => "medium",
            Preset::Slow => "slow",
            Preset::VerySlow => "veryslow",
        }
    }
}

/// 码率控制模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateControl {
    /// 恒定码率（CBR）
    CBR,
    /// 可变码率（VBR）
    VBR,
    /// 恒定质量（CRF）
    CRF { value: u32 },
}

impl Default for FfmpegConfig {
    fn default() -> Self {
        Self::balanced()
    }
}

impl FfmpegConfig {
    /// 平衡配置（推荐）
    pub fn balanced() -> Self {
        Self {
            threads: 0,  // 自动
            buffer_size: 8 * 1024 * 1024,  // 8MB
            gop_size: 60,  // 2秒 @ 30fps
            b_frames: 2,
            ref_frames: 3,
            preset: Preset::Fast,
            rate_control: RateControl::VBR,
            low_latency: false,
            zero_copy: false,
        }
    }

    /// 低延迟配置（适用于实时监控）
    pub fn low_latency() -> Self {
        Self {
            threads: 0,
            buffer_size: 2 * 1024 * 1024,  // 2MB
            gop_size: 30,  // 1秒 @ 30fps
            b_frames: 0,  // 禁用 B 帧
            ref_frames: 1,
            preset: Preset::VeryFast,
            rate_control: RateControl::CBR,
            low_latency: true,
            zero_copy: true,
        }
    }

    /// 高质量配置（适用于录像）
    pub fn high_quality() -> Self {
        Self {
            threads: 0,
            buffer_size: 16 * 1024 * 1024,  // 16MB
            gop_size: 120,  // 4秒 @ 30fps
            b_frames: 3,
            ref_frames: 5,
            preset: Preset::Slow,
            rate_control: RateControl::CRF { value: 23 },
            low_latency: false,
            zero_copy: false,
        }
    }

    /// 高性能配置（适用于大规模并发）
    pub fn high_performance() -> Self {
        Self {
            threads: 0,
            buffer_size: 4 * 1024 * 1024,  // 4MB
            gop_size: 60,
            b_frames: 0,  // 禁用 B 帧
            ref_frames: 1,
            preset: Preset::UltraFast,
            rate_control: RateControl::VBR,
            low_latency: false,
            zero_copy: true,
        }
    }

    /// 根据硬件加速类型优化配置
    pub fn optimize_for_hw(&mut self, hw_accel: &HardwareAccel) {
        match hw_accel {
            HardwareAccel::NVENC => {
                // NVIDIA GPU 优化
                self.preset = Preset::Fast;
                self.b_frames = 2;
                self.ref_frames = 3;
                self.zero_copy = true;
            }
            HardwareAccel::QSV => {
                // Intel QSV 优化
                self.preset = Preset::Fast;
                self.b_frames = 2;
                self.ref_frames = 2;
                self.zero_copy = true;
            }
            HardwareAccel::VideoToolbox => {
                // Apple VideoToolbox 优化
                self.preset = Preset::Medium;
                self.b_frames = 0;
                self.ref_frames = 1;
            }
            HardwareAccel::VAAPI => {
                // Linux VAAPI 优化
                self.preset = Preset::Fast;
                self.b_frames = 1;
                self.ref_frames = 2;
            }
        }
    }

    /// 生成 FFmpeg 命令行参数
    pub fn to_ffmpeg_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // 线程数
        if self.threads > 0 {
            args.push("-threads".to_string());
            args.push(self.threads.to_string());
        }

        // 缓冲区大小
        args.push("-bufsize".to_string());
        args.push(self.buffer_size.to_string());

        // GOP 大小
        args.push("-g".to_string());
        args.push(self.gop_size.to_string());

        // B 帧
        args.push("-bf".to_string());
        args.push(self.b_frames.to_string());

        // 参考帧
        args.push("-refs".to_string());
        args.push(self.ref_frames.to_string());

        // 预设
        args.push("-preset".to_string());
        args.push(self.preset.as_str().to_string());

        // 码率控制
        match self.rate_control {
            RateControl::CBR => {
                args.push("-rc".to_string());
                args.push("cbr".to_string());
            }
            RateControl::VBR => {
                args.push("-rc".to_string());
                args.push("vbr".to_string());
            }
            RateControl::CRF { value } => {
                args.push("-crf".to_string());
                args.push(value.to_string());
            }
        }

        // 低延迟选项
        if self.low_latency {
            args.push("-tune".to_string());
            args.push("zerolatency".to_string());
            args.push("-fflags".to_string());
            args.push("nobuffer".to_string());
            args.push("-flags".to_string());
            args.push("low_delay".to_string());
        }

        // 零拷贝
        if self.zero_copy {
            args.push("-hwaccel_output_format".to_string());
            args.push("cuda".to_string());
        }

        args
    }
}

/// 场景配置
pub struct ScenarioConfig;

impl ScenarioConfig {
    /// 内网监控（300路）
    pub fn internal_monitoring() -> FfmpegConfig {
        let mut config = FfmpegConfig::high_performance();
        config.gop_size = 30;  // 1秒关键帧
        config.buffer_size = 2 * 1024 * 1024;
        config
    }

    /// 互联网直播（低延迟）
    pub fn live_streaming() -> FfmpegConfig {
        FfmpegConfig::low_latency()
    }

    /// 录像存储（高质量）
    pub fn recording() -> FfmpegConfig {
        FfmpegConfig::high_quality()
    }

    /// 移动端推流（省电）
    pub fn mobile_streaming() -> FfmpegConfig {
        let mut config = FfmpegConfig::balanced();
        config.preset = Preset::VeryFast;
        config.b_frames = 0;
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_conversion() {
        assert_eq!(Preset::Fast.as_str(), "fast");
        assert_eq!(Preset::UltraFast.as_str(), "ultrafast");
    }

    #[test]
    fn test_balanced_config() {
        let config = FfmpegConfig::balanced();
        assert_eq!(config.preset, Preset::Fast);
        assert_eq!(config.b_frames, 2);
    }

    #[test]
    fn test_low_latency_config() {
        let config = FfmpegConfig::low_latency();
        assert_eq!(config.b_frames, 0);
        assert!(config.low_latency);
        assert!(config.zero_copy);
    }

    #[test]
    fn test_hw_optimization() {
        let mut config = FfmpegConfig::balanced();
        config.optimize_for_hw(&HardwareAccel::NVENC);
        assert!(config.zero_copy);
        assert_eq!(config.ref_frames, 3);
    }

    #[test]
    fn test_ffmpeg_args_generation() {
        let config = FfmpegConfig::low_latency();
        let args = config.to_ffmpeg_args();
        
        assert!(args.contains(&"-tune".to_string()));
        assert!(args.contains(&"zerolatency".to_string()));
        assert!(args.contains(&"-bf".to_string()));
        assert!(args.contains(&"0".to_string()));
    }

    #[test]
    fn test_scenario_configs() {
        let monitoring = ScenarioConfig::internal_monitoring();
        assert_eq!(monitoring.gop_size, 30);
        
        let streaming = ScenarioConfig::live_streaming();
        assert!(streaming.low_latency);
        
        let recording = ScenarioConfig::recording();
        assert_eq!(recording.preset, Preset::Slow);
    }
}
