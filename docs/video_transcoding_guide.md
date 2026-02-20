# 视频转码技术指南

**更新时间**: 2026-02-19 18:10 UTC+08:00  
**状态**: ✅ **完整转码技术文档**

---

## 🎯 转码需求

摄像头输入的视频流有各种分辨率和码率：
- 低端摄像头：640×480, 720×576
- 标准摄像头：1280×720 (720p)
- 高清摄像头：1920×1080 (1080p)
- 4K摄像头：3840×2160 (4K)
- 各种码率：0.5-8 Mbps

需要统一转换成标准格式以便存储和播放。

---

## 🔧 转码技术方案

### 方案 1: FFmpeg（推荐）✅

FFmpeg 是业界标准的视频处理工具，支持硬件加速。

#### 安装 FFmpeg

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
apt-get install ffmpeg

# 带硬件加速支持
apt-get install ffmpeg libva-dev libvdpau-dev
```

#### 基本转码命令

```bash
# 转换到 1080p, 2 Mbps, 25 fps
ffmpeg -i input.mp4 \
  -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2" \
  -c:v libx264 \
  -b:v 2000k \
  -r 25 \
  -preset medium \
  -profile:v high \
  -c:a aac \
  -b:a 128k \
  output.mp4
```

**参数说明**：
- `-vf "scale=..."`: 缩放到 1920×1080，保持宽高比，黑边填充
- `-c:v libx264`: 使用 H.264 编码器
- `-b:v 2000k`: 视频码率 2 Mbps
- `-r 25`: 帧率 25 fps
- `-preset medium`: 编码速度（ultrafast/fast/medium/slow）
- `-profile:v high`: H.264 High Profile
- `-c:a aac`: 音频编码器 AAC
- `-b:a 128k`: 音频码率 128 kbps

---

### 方案 2: 使用 Rust FFmpeg 绑定

#### 依赖库

```toml
[dependencies]
ffmpeg-next = "6.0"
```

#### Rust 实现

```rust
use ffmpeg_next as ffmpeg;
use std::path::Path;

pub struct VideoTranscoder {
    input_ctx: ffmpeg::format::context::Input,
    output_ctx: ffmpeg::format::context::Output,
}

impl VideoTranscoder {
    /// 转码到指定质量
    pub fn transcode(
        input_path: &Path,
        output_path: &Path,
        target_quality: Quality,
    ) -> Result<()> {
        ffmpeg::init()?;
        
        let params = target_quality.get_params();
        
        // 打开输入文件
        let mut input = ffmpeg::format::input(input_path)?;
        
        // 创建输出文件
        let mut output = ffmpeg::format::output(output_path)?;
        
        // 查找视频流
        let video_stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(anyhow!("No video stream"))?;
        
        let video_index = video_stream.index();
        
        // 创建解码器
        let decoder = video_stream.codec().decoder().video()?;
        
        // 创建编码器
        let mut encoder = ffmpeg::codec::encoder::find(ffmpeg::codec::Id::H264)
            .ok_or(anyhow!("H264 encoder not found"))?
            .video()?;
        
        // 设置编码参数
        encoder.set_width(params.width);
        encoder.set_height(params.height);
        encoder.set_bit_rate(params.video_bitrate * 1000);
        encoder.set_frame_rate(Some((params.framerate, 1).into()));
        encoder.set_format(ffmpeg::format::Pixel::YUV420P);
        
        let encoder = encoder.open()?;
        
        // 添加输出流
        let mut out_stream = output.add_stream(encoder)?;
        out_stream.set_parameters(&encoder);
        
        // 写入文件头
        output.write_header()?;
        
        // 创建缩放器（用于分辨率转换）
        let mut scaler = ffmpeg::software::scaling::Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            ffmpeg::format::Pixel::YUV420P,
            params.width,
            params.height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?;
        
        // 转码循环
        for (stream, packet) in input.packets() {
            if stream.index() == video_index {
                // 解码
                decoder.send_packet(&packet)?;
                
                let mut decoded = ffmpeg::util::frame::Video::empty();
                while decoder.receive_frame(&mut decoded).is_ok() {
                    // 缩放
                    let mut scaled = ffmpeg::util::frame::Video::empty();
                    scaler.run(&decoded, &mut scaled)?;
                    
                    // 编码
                    encoder.send_frame(&scaled)?;
                    
                    let mut encoded = ffmpeg::Packet::empty();
                    while encoder.receive_packet(&mut encoded).is_ok() {
                        encoded.set_stream(0);
                        encoded.write_interleaved(&mut output)?;
                    }
                }
            }
        }
        
        // 刷新编码器
        encoder.send_eof()?;
        let mut encoded = ffmpeg::Packet::empty();
        while encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(0);
            encoded.write_interleaved(&mut output)?;
        }
        
        // 写入文件尾
        output.write_trailer()?;
        
        Ok(())
    }
}
```

---

## 📊 不同输入分辨率的转换策略

### 1. 放大（Upscaling）

**输入**: 720p (1280×720) → **输出**: 1080p (1920×1080)

```bash
ffmpeg -i input_720p.mp4 \
  -vf "scale=1920:1080:flags=lanczos" \
  -c:v libx264 -b:v 2000k -r 25 \
  output_1080p.mp4
```

**注意**：
- ⚠️ 放大不会增加实际清晰度
- 使用 Lanczos 算法获得最佳质量
- 建议保持原始分辨率或使用 `original` 质量

---

### 2. 缩小（Downscaling）

**输入**: 4K (3840×2160) → **输出**: 1080p (1920×1080)

```bash
ffmpeg -i input_4k.mp4 \
  -vf "scale=1920:1080:flags=lanczos" \
  -c:v libx264 -b:v 2000k -r 25 \
  output_1080p.mp4
```

**优势**：
- ✅ 减少文件大小
- ✅ 保持良好画质
- ✅ 降低播放设备要求

---

### 3. 保持宽高比

**输入**: 16:9 或 4:3 → **输出**: 16:9 (1920×1080)

```bash
# 保持宽高比，黑边填充
ffmpeg -i input.mp4 \
  -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:black" \
  -c:v libx264 -b:v 2000k -r 25 \
  output.mp4
```

**效果**：
```
输入 4:3 (1024×768)
  ↓ 缩放到 1440×1080
  ↓ 左右填充黑边
输出 16:9 (1920×1080)
```

---

### 4. 裁剪（Cropping）

**输入**: 4:3 → **输出**: 16:9 (裁剪)

```bash
# 裁剪到 16:9
ffmpeg -i input_4_3.mp4 \
  -vf "crop=ih*16/9:ih,scale=1920:1080" \
  -c:v libx264 -b:v 2000k -r 25 \
  output.mp4
```

---

## 🚀 硬件加速转码

### NVIDIA GPU (NVENC)

```bash
# 使用 NVIDIA 硬件加速
ffmpeg -hwaccel cuda -i input.mp4 \
  -vf "scale_cuda=1920:1080" \
  -c:v h264_nvenc \
  -b:v 2000k \
  -preset p4 \
  output.mp4
```

**优势**：
- ✅ 速度提升 5-10x
- ✅ CPU 占用降低 90%
- ✅ 支持多路并发

---

### Intel Quick Sync (QSV)

```bash
# 使用 Intel 硬件加速
ffmpeg -hwaccel qsv -i input.mp4 \
  -vf "scale_qsv=1920:1080" \
  -c:v h264_qsv \
  -b:v 2000k \
  output.mp4
```

---

### Apple VideoToolbox (macOS)

```bash
# 使用 Apple 硬件加速
ffmpeg -hwaccel videotoolbox -i input.mp4 \
  -vf "scale=1920:1080" \
  -c:v h264_videotoolbox \
  -b:v 2000k \
  output.mp4
```

---

## 🔄 实时流转码

### RTMP 流转码

```bash
# 接收 RTMP 流，转码后输出
ffmpeg -i rtmp://source/live/stream \
  -vf "scale=1920:1080" \
  -c:v libx264 -b:v 2000k -r 25 -preset ultrafast \
  -c:a aac -b:a 128k \
  -f flv rtmp://output/live/stream
```

**实时转码优化**：
- 使用 `ultrafast` 预设（降低延迟）
- 使用硬件加速
- 降低 GOP 大小

---

## 💻 Rust 完整实现

```rust
use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

pub struct VideoTranscoder {
    ffmpeg_path: PathBuf,
    hardware_accel: Option<HardwareAccel>,
}

pub enum HardwareAccel {
    Nvidia,   // NVENC
    Intel,    // QSV
    Apple,    // VideoToolbox
}

impl VideoTranscoder {
    pub fn new() -> Self {
        Self {
            ffmpeg_path: PathBuf::from("ffmpeg"),
            hardware_accel: Self::detect_hardware_accel(),
        }
    }
    
    /// 检测可用的硬件加速
    fn detect_hardware_accel() -> Option<HardwareAccel> {
        // 检测 NVIDIA GPU
        if Command::new("nvidia-smi").output().is_ok() {
            return Some(HardwareAccel::Nvidia);
        }
        
        // 检测 Intel QSV
        #[cfg(target_os = "linux")]
        if std::path::Path::new("/dev/dri/renderD128").exists() {
            return Some(HardwareAccel::Intel);
        }
        
        // 检测 Apple VideoToolbox
        #[cfg(target_os = "macos")]
        return Some(HardwareAccel::Apple);
        
        None
    }
    
    /// 转码视频
    pub async fn transcode(
        &self,
        input: &PathBuf,
        output: &PathBuf,
        quality: Quality,
    ) -> Result<()> {
        let params = quality.get_params();
        
        let mut cmd = Command::new(&self.ffmpeg_path);
        
        // 硬件加速
        if let Some(ref hw) = self.hardware_accel {
            match hw {
                HardwareAccel::Nvidia => {
                    cmd.args(&["-hwaccel", "cuda"]);
                    cmd.args(&["-c:v", "h264_nvenc"]);
                }
                HardwareAccel::Intel => {
                    cmd.args(&["-hwaccel", "qsv"]);
                    cmd.args(&["-c:v", "h264_qsv"]);
                }
                HardwareAccel::Apple => {
                    cmd.args(&["-hwaccel", "videotoolbox"]);
                    cmd.args(&["-c:v", "h264_videotoolbox"]);
                }
            }
        } else {
            cmd.args(&["-c:v", "libx264"]);
        }
        
        // 输入文件
        cmd.args(&["-i", input.to_str().unwrap()]);
        
        // 视频滤镜（缩放）
        let scale_filter = format!(
            "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
            params.width, params.height, params.width, params.height
        );
        cmd.args(&["-vf", &scale_filter]);
        
        // 视频参数
        cmd.args(&["-b:v", &format!("{}k", params.video_bitrate)]);
        cmd.args(&["-r", &params.framerate.to_string()]);
        cmd.args(&["-preset", params.preset]);
        
        // 音频参数
        cmd.args(&["-c:a", "aac"]);
        cmd.args(&["-b:a", &format!("{}k", params.audio_bitrate)]);
        
        // 输出文件
        cmd.arg(output.to_str().unwrap());
        
        // 执行转码
        let output = cmd.output()?;
        
        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "FFmpeg failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        
        Ok(())
    }
}
```

---

## 📊 转码性能对比

### 软件编码 vs 硬件加速

| 方案 | 速度 | CPU占用 | 画质 | 并发能力 |
|------|------|---------|------|---------|
| **软件 (libx264)** | 1x | 100% | 最好 | 1-2路 |
| **NVIDIA (NVENC)** | 5-10x | 10% | 良好 | 10-20路 |
| **Intel (QSV)** | 3-5x | 20% | 良好 | 5-10路 |
| **Apple (VT)** | 4-6x | 15% | 良好 | 5-10路 |

---

## 🎯 推荐方案

### 实时录像（不转码）

```toml
[recording.quality]
realtime = "original"  # 保持原始质量，不转码
```

**优势**：
- ✅ 零延迟
- ✅ 零 CPU 占用
- ✅ 原始画质

---

### 归档转换（后台转码）

```toml
[recording.conversion]
enabled = true
trigger_after_hours = 24
target_quality = "medium"  # 转码到 720p
```

**流程**：
```
实时录像（原始质量）
  ↓ 24小时后
后台转码任务
  ↓ 使用硬件加速
归档存储（720p, 1 Mbps）
```

---

## 🎯 总结

**转码方案**：
1. ✅ **实时录像**: 保持原始质量（不转码）
2. ✅ **后台转换**: 24小时后自动转码
3. ✅ **硬件加速**: 使用 GPU 加速（5-10x 速度）
4. ✅ **智能缩放**: 保持宽高比，黑边填充

**推荐配置**：
```toml
realtime = "original"      # 实时不转码
archive = "medium"         # 归档转到 720p
trigger_after_hours = 24   # 24小时后转码
```

这样既保证了实时录像的零延迟，又通过后台转码节省了存储空间！🚀

---

**文档完成时间**: 2026-02-19 18:10 UTC+08:00  
**状态**: ✅ **完整转码技术文档**
