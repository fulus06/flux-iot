use anyhow::{anyhow, Result};
use flux_media_core::types::StreamId;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use super::config::FfmpegConfig;

/// 直通模式处理器（零转码）
/// 使用 FFmpeg -c:v copy -c:a copy 进行流复制
pub struct PassthroughProcessor {
    stream_id: StreamId,
    input_url: String,
    output_configs: Vec<OutputConfig>,
    ffmpeg_config: Option<FfmpegConfig>,
    process: Arc<RwLock<Option<Child>>>,
}

#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    HLS,
    FLV,
    RTMP,
    RTSP,
}

impl PassthroughProcessor {
    pub fn new(stream_id: StreamId, input_url: String, output_configs: Vec<OutputConfig>) -> Self {
        Self {
            stream_id,
            input_url,
            output_configs,
            ffmpeg_config: None,
            process: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_config(mut self, config: FfmpegConfig) -> Self {
        self.ffmpeg_config = Some(config);
        self
    }

    /// 启动直通处理
    pub async fn start(&self) -> Result<()> {
        info!(
            stream_id = %self.stream_id,
            input = %self.input_url,
            outputs = self.output_configs.len(),
            "Starting passthrough processor"
        );

        let mut cmd = Command::new("ffmpeg");
        
        // 应用性能配置
        if let Some(ref config) = self.ffmpeg_config {
            for arg in config.to_ffmpeg_args() {
                cmd.arg(arg);
            }
        }
        
        // 输入配置
        cmd.arg("-i").arg(&self.input_url);
        
        // 全局选项
        cmd.arg("-c:v").arg("copy");  // 视频流复制（零转码）
        cmd.arg("-c:a").arg("copy");  // 音频流复制（零转码）
        cmd.arg("-f").arg("tee");     // 使用 tee muxer 支持多输出
        
        // 构建输出 URL 列表
        let output_urls = self.build_output_urls();
        cmd.arg(&output_urls);
        
        // 日志配置
        cmd.arg("-loglevel").arg("warning");
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        // 启动进程
        let child = cmd.spawn().map_err(|e| {
            error!(stream_id = %self.stream_id, error = %e, "Failed to spawn ffmpeg");
            anyhow!("Failed to spawn ffmpeg: {}", e)
        })?;

        let mut process = self.process.write().await;
        *process = Some(child);

        info!(stream_id = %self.stream_id, "Passthrough processor started");
        Ok(())
    }

    /// 停止直通处理
    pub async fn stop(&self) -> Result<()> {
        info!(stream_id = %self.stream_id, "Stopping passthrough processor");

        let mut process = self.process.write().await;
        if let Some(mut child) = process.take() {
            child.kill().map_err(|e| {
                error!(stream_id = %self.stream_id, error = %e, "Failed to kill ffmpeg");
                anyhow!("Failed to kill ffmpeg: {}", e)
            })?;
            
            child.wait().map_err(|e| {
                error!(stream_id = %self.stream_id, error = %e, "Failed to wait for ffmpeg");
                anyhow!("Failed to wait for ffmpeg: {}", e)
            })?;
        }

        info!(stream_id = %self.stream_id, "Passthrough processor stopped");
        Ok(())
    }

    /// 检查进程是否运行
    pub async fn is_running(&self) -> bool {
        let process = self.process.read().await;
        process.is_some()
    }

    /// 构建输出 URL 列表（tee muxer 格式）
    fn build_output_urls(&self) -> String {
        self.output_configs
            .iter()
            .map(|config| {
                let format = match config.format {
                    OutputFormat::HLS => "hls",
                    OutputFormat::FLV => "flv",
                    OutputFormat::RTMP => "flv",
                    OutputFormat::RTSP => "rtsp",
                };
                format!("[f={}]{}",format, config.url)
            })
            .collect::<Vec<_>>()
            .join("|")
    }
}

impl Drop for PassthroughProcessor {
    fn drop(&mut self) {
        // 确保进程被清理
        if let Ok(mut process) = self.process.try_write() {
            if let Some(mut child) = process.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_output_urls() {
        let stream_id = StreamId::new("test", "stream-001");
        let output_configs = vec![
            OutputConfig {
                format: OutputFormat::HLS,
                url: "/tmp/hls/stream.m3u8".to_string(),
            },
            OutputConfig {
                format: OutputFormat::FLV,
                url: "rtmp://localhost/live/stream".to_string(),
            },
        ];

        let processor = PassthroughProcessor::new(
            stream_id,
            "rtsp://localhost/stream".to_string(),
            output_configs,
        );

        let urls = processor.build_output_urls();
        assert!(urls.contains("[f=hls]"));
        assert!(urls.contains("[f=flv]"));
        assert!(urls.contains("|"));
    }

    #[tokio::test]
    async fn test_processor_lifecycle() {
        let stream_id = StreamId::new("test", "stream-001");
        let processor = PassthroughProcessor::new(
            stream_id,
            "rtsp://localhost/stream".to_string(),
            vec![],
        );

        assert!(!processor.is_running().await);
    }
}
