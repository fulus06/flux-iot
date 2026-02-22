use anyhow::{anyhow, Result};
use flux_config::{BitrateConfig, HardwareAccel};
use flux_media_core::types::StreamId;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use super::config::FfmpegConfig;

/// 转码处理器（多码率转码）
pub struct TranscodeProcessor {
    stream_id: StreamId,
    input_url: String,
    bitrates: Vec<BitrateConfig>,
    hw_accel: Option<HardwareAccel>,
    output_dir: String,
    ffmpeg_config: FfmpegConfig,
    process: Arc<RwLock<Option<Child>>>,
}

impl TranscodeProcessor {
    pub fn new(
        stream_id: StreamId,
        input_url: String,
        bitrates: Vec<BitrateConfig>,
        hw_accel: Option<HardwareAccel>,
        output_dir: String,
    ) -> Self {
        let mut ffmpeg_config = FfmpegConfig::balanced();
        if let Some(ref hw) = hw_accel {
            ffmpeg_config.optimize_for_hw(hw);
        }
        
        Self {
            stream_id,
            input_url,
            bitrates,
            hw_accel,
            output_dir,
            ffmpeg_config,
            process: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_config(mut self, config: FfmpegConfig) -> Self {
        self.ffmpeg_config = config;
        self
    }

    /// 启动转码处理
    pub async fn start(&self) -> Result<()> {
        info!(
            stream_id = %self.stream_id,
            input = %self.input_url,
            bitrates = self.bitrates.len(),
            hw_accel = ?self.hw_accel,
            "Starting transcode processor"
        );

        let mut cmd = Command::new("ffmpeg");
        
        // 硬件加速配置
        if let Some(hw_accel) = &self.hw_accel {
            self.add_hw_accel_args(&mut cmd, hw_accel);
        }
        
        // 输入配置
        cmd.arg("-i").arg(&self.input_url);
        
        // 应用性能配置
        for arg in self.ffmpeg_config.to_ffmpeg_args() {
            cmd.arg(arg);
        }
        
        // 为每个码率生成输出
        for (idx, bitrate) in self.bitrates.iter().enumerate() {
            self.add_bitrate_output(&mut cmd, bitrate, idx);
        }
        
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

        info!(stream_id = %self.stream_id, "Transcode processor started");
        Ok(())
    }

    /// 停止转码处理
    pub async fn stop(&self) -> Result<()> {
        info!(stream_id = %self.stream_id, "Stopping transcode processor");

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

        info!(stream_id = %self.stream_id, "Transcode processor stopped");
        Ok(())
    }

    /// 检查进程是否运行
    pub async fn is_running(&self) -> bool {
        let process = self.process.read().await;
        process.is_some()
    }

    /// 添加硬件加速参数
    fn add_hw_accel_args(&self, cmd: &mut Command, hw_accel: &HardwareAccel) {
        match hw_accel {
            HardwareAccel::NVENC => {
                cmd.arg("-hwaccel").arg("cuda");
                cmd.arg("-hwaccel_output_format").arg("cuda");
            }
            HardwareAccel::QSV => {
                cmd.arg("-hwaccel").arg("qsv");
                cmd.arg("-hwaccel_output_format").arg("qsv");
            }
            HardwareAccel::VideoToolbox => {
                cmd.arg("-hwaccel").arg("videotoolbox");
            }
            HardwareAccel::VAAPI => {
                cmd.arg("-hwaccel").arg("vaapi");
                cmd.arg("-hwaccel_device").arg("/dev/dri/renderD128");
            }
        }
    }

    /// 添加码率输出配置
    fn add_bitrate_output(&self, cmd: &mut Command, bitrate: &BitrateConfig, idx: usize) {
        let (width, height) = bitrate.resolution;
        
        // 视频编码器
        let encoder = self.get_video_encoder();
        
        // 视频流配置
        cmd.arg(format!("-map")).arg("0:v:0");
        cmd.arg(format!("-c:v:{}", idx)).arg(&encoder);
        cmd.arg(format!("-b:v:{}", idx)).arg(format!("{}k", bitrate.bitrate));
        cmd.arg(format!("-s:v:{}", idx)).arg(format!("{}x{}", width, height));
        cmd.arg(format!("-r:{}", idx)).arg(format!("{}", bitrate.framerate));
        cmd.arg(format!("-preset:{}", idx)).arg(&bitrate.encoder_preset);
        
        // 音频流配置
        cmd.arg(format!("-map")).arg("0:a:0?");
        cmd.arg(format!("-c:a:{}", idx)).arg("aac");
        cmd.arg(format!("-b:a:{}", idx)).arg("128k");
        
        // HLS 输出
        cmd.arg(format!("-f")).arg("hls");
        cmd.arg(format!("-hls_time")).arg("6");
        cmd.arg(format!("-hls_list_size")).arg("10");
        cmd.arg(format!("-hls_flags")).arg("delete_segments");
        
        let output_path = format!("{}/{}.m3u8", self.output_dir, bitrate.name);
        cmd.arg(&output_path);
    }

    /// 获取视频编码器
    fn get_video_encoder(&self) -> String {
        match &self.hw_accel {
            Some(HardwareAccel::NVENC) => "h264_nvenc".to_string(),
            Some(HardwareAccel::QSV) => "h264_qsv".to_string(),
            Some(HardwareAccel::VideoToolbox) => "h264_videotoolbox".to_string(),
            Some(HardwareAccel::VAAPI) => "h264_vaapi".to_string(),
            None => "libx264".to_string(),
        }
    }
}

impl Drop for TranscodeProcessor {
    fn drop(&mut self) {
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
    fn test_get_video_encoder() {
        let stream_id = StreamId::new("test", "stream-001");
        
        let processor = TranscodeProcessor::new(
            stream_id.clone(),
            "rtsp://localhost/stream".to_string(),
            vec![],
            Some(HardwareAccel::NVENC),
            "/tmp".to_string(),
        );
        assert_eq!(processor.get_video_encoder(), "h264_nvenc");
        
        let processor = TranscodeProcessor::new(
            stream_id.clone(),
            "rtsp://localhost/stream".to_string(),
            vec![],
            None,
            "/tmp".to_string(),
        );
        assert_eq!(processor.get_video_encoder(), "libx264");
    }

    #[tokio::test]
    async fn test_processor_lifecycle() {
        let stream_id = StreamId::new("test", "stream-001");
        let processor = TranscodeProcessor::new(
            stream_id,
            "rtsp://localhost/stream".to_string(),
            vec![],
            None,
            "/tmp".to_string(),
        );

        assert!(!processor.is_running().await);
    }
}
