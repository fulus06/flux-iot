use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 多码率配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultibitrateConfig {
    /// 码率变体列表
    pub variants: Vec<BitrateVariant>,
    
    /// 默认码率索引
    pub default_index: usize,
}

/// 码率变体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitrateVariant {
    /// 变体名称
    pub name: String,
    
    /// 码率（kbps）
    pub bitrate: u32,
    
    /// 分辨率
    pub resolution: (u32, u32),
    
    /// 帧率
    pub framerate: f32,
    
    /// 编码器配置
    pub encoder_preset: String,
}

impl Default for MultibitrateConfig {
    fn default() -> Self {
        Self {
            variants: vec![
                BitrateVariant {
                    name: "low".to_string(),
                    bitrate: 500,
                    resolution: (640, 360),
                    framerate: 25.0,
                    encoder_preset: "veryfast".to_string(),
                },
                BitrateVariant {
                    name: "medium".to_string(),
                    bitrate: 1000,
                    resolution: (1280, 720),
                    framerate: 25.0,
                    encoder_preset: "fast".to_string(),
                },
                BitrateVariant {
                    name: "high".to_string(),
                    bitrate: 2000,
                    resolution: (1920, 1080),
                    framerate: 30.0,
                    encoder_preset: "medium".to_string(),
                },
            ],
            default_index: 1,
        }
    }
}

/// HLS Master Playlist 生成器
pub struct MasterPlaylistGenerator {
    variants: Vec<BitrateVariant>,
}

impl MasterPlaylistGenerator {
    pub fn new(variants: Vec<BitrateVariant>) -> Self {
        Self { variants }
    }

    /// 生成 Master Playlist
    pub fn generate(&self, base_url: &str) -> String {
        let mut m3u8 = String::new();
        
        m3u8.push_str("#EXTM3U\n");
        m3u8.push_str("#EXT-X-VERSION:3\n\n");

        for variant in &self.variants {
            // Stream info
            m3u8.push_str(&format!(
                "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}x{},FRAME-RATE={:.3}\n",
                variant.bitrate * 1000,
                variant.resolution.0,
                variant.resolution.1,
                variant.framerate
            ));
            
            // Variant playlist URL
            m3u8.push_str(&format!("{}/{}/playlist.m3u8\n\n", base_url, variant.name));
        }

        m3u8
    }

    /// 生成 DASH MPD
    pub fn generate_dash_mpd(&self, base_url: &str) -> String {
        let mut mpd = String::new();
        
        mpd.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        mpd.push_str("<MPD xmlns=\"urn:mpeg:dash:schema:mpd:2011\" ");
        mpd.push_str("type=\"dynamic\" ");
        mpd.push_str("minimumUpdatePeriod=\"PT2S\" ");
        mpd.push_str("minBufferTime=\"PT4S\">\n");
        
        mpd.push_str("  <Period>\n");
        mpd.push_str("    <AdaptationSet mimeType=\"video/mp4\" ");
        mpd.push_str("codecs=\"avc1.64001f\">\n");

        for variant in &self.variants {
            mpd.push_str(&format!(
                "      <Representation id=\"{}\" bandwidth=\"{}\" width=\"{}\" height=\"{}\">\n",
                variant.name,
                variant.bitrate * 1000,
                variant.resolution.0,
                variant.resolution.1
            ));
            mpd.push_str(&format!(
                "        <BaseURL>{}/{}/</BaseURL>\n",
                base_url, variant.name
            ));
            mpd.push_str("        <SegmentTemplate media=\"segment_$Number$.m4s\" ");
            mpd.push_str("initialization=\"init.mp4\" ");
            mpd.push_str("timescale=\"1000\" ");
            mpd.push_str("duration=\"4000\"/>\n");
            mpd.push_str("      </Representation>\n");
        }

        mpd.push_str("    </AdaptationSet>\n");
        mpd.push_str("  </Period>\n");
        mpd.push_str("</MPD>\n");

        mpd
    }
}

/// 多码率流管理器
pub struct MultibitrateStreamManager {
    config: MultibitrateConfig,
    active_streams: HashMap<String, usize>, // stream_id -> variant_index
}

impl MultibitrateStreamManager {
    pub fn new(config: MultibitrateConfig) -> Self {
        Self {
            config,
            active_streams: HashMap::new(),
        }
    }

    /// 注册流
    pub fn register_stream(&mut self, stream_id: String) {
        self.active_streams
            .insert(stream_id, self.config.default_index);
    }

    /// 切换码率
    pub fn switch_bitrate(&mut self, stream_id: &str, variant_index: usize) -> Option<&BitrateVariant> {
        if variant_index < self.config.variants.len() {
            self.active_streams.insert(stream_id.to_string(), variant_index);
            Some(&self.config.variants[variant_index])
        } else {
            None
        }
    }

    /// 获取当前变体
    pub fn get_current_variant(&self, stream_id: &str) -> Option<&BitrateVariant> {
        self.active_streams
            .get(stream_id)
            .and_then(|&index| self.config.variants.get(index))
    }

    /// 获取所有变体
    pub fn get_variants(&self) -> &[BitrateVariant] {
        &self.config.variants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multibitrate_config_default() {
        let config = MultibitrateConfig::default();
        assert_eq!(config.variants.len(), 3);
        assert_eq!(config.default_index, 1);
    }

    #[test]
    fn test_master_playlist_generation() {
        let config = MultibitrateConfig::default();
        let generator = MasterPlaylistGenerator::new(config.variants);
        
        let playlist = generator.generate("http://example.com/stream");
        
        assert!(playlist.contains("#EXTM3U"));
        assert!(playlist.contains("#EXT-X-STREAM-INF"));
        assert!(playlist.contains("BANDWIDTH=500000"));
        assert!(playlist.contains("BANDWIDTH=1000000"));
        assert!(playlist.contains("BANDWIDTH=2000000"));
    }

    #[test]
    fn test_stream_manager() {
        let config = MultibitrateConfig::default();
        let mut manager = MultibitrateStreamManager::new(config);
        
        manager.register_stream("stream1".to_string());
        
        let variant = manager.get_current_variant("stream1").unwrap();
        assert_eq!(variant.name, "medium");
        
        manager.switch_bitrate("stream1", 2);
        let variant = manager.get_current_variant("stream1").unwrap();
        assert_eq!(variant.name, "high");
    }

    #[test]
    fn test_dash_mpd_generation() {
        let config = MultibitrateConfig::default();
        let generator = MasterPlaylistGenerator::new(config.variants);
        
        let mpd = generator.generate_dash_mpd("http://example.com/stream");
        
        assert!(mpd.contains("<?xml"));
        assert!(mpd.contains("<MPD"));
        assert!(mpd.contains("Representation"));
        assert!(mpd.contains("bandwidth=\"500000\""));
    }
}
