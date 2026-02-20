# 增强型存储系统设计

**设计时间**: 2026-02-19 19:10 UTC+08:00  
**状态**: 📋 **增强设计**

---

## 🎯 设计目标

参考 MinIO 的企业级存储功能，为 FLUX IOT 设计一个健壮的存储系统。

### 核心功能

1. **磁盘健康检测** - 实时监控磁盘状态
2. **存储池管理** - 多磁盘负载均衡
3. **智能转码** - 根据画质自动决定是否转码
4. **容量管理** - 自动清理和告警
5. **数据完整性** - 校验和验证
6. **性能监控** - I/O 统计和优化

---

## 🏗️ 系统架构

```
┌─────────────────────────────────────────────────────────┐
│              存储管理层                                  │
│  ┌──────────────────────────────────────────────┐      │
│  │  StorageManager（存储管理器）                 │      │
│  │  - 磁盘检测和监控                            │      │
│  │  - 存储池管理                                │      │
│  │  - 负载均衡                                  │      │
│  └──────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│              磁盘健康层                                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐               │
│  │  Disk 1  │ │  Disk 2  │ │  Disk 3  │               │
│  │  SSD     │ │  HDD     │ │  HDD     │               │
│  │  健康    │ │  健康    │ │  警告    │               │
│  └──────────┘ └──────────┘ └──────────┘               │
└─────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│              数据完整性层                                │
│  - 校验和验证                                           │
│  - 数据修复                                             │
│  - 冗余备份                                             │
└─────────────────────────────────────────────────────────┘
```

---

## 💻 核心组件设计

### 1. 存储管理器

```rust
use sysinfo::{System, SystemExt, DiskExt};
use std::path::PathBuf;
use std::collections::HashMap;

/// 存储管理器
pub struct StorageManager {
    /// 存储池
    pools: HashMap<String, StoragePool>,
    
    /// 磁盘监控器
    disk_monitor: DiskMonitor,
    
    /// 健康检查器
    health_checker: HealthChecker,
    
    /// 性能统计
    metrics: StorageMetrics,
}

impl StorageManager {
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            disk_monitor: DiskMonitor::new(),
            health_checker: HealthChecker::new(),
            metrics: StorageMetrics::new(),
        }
    }
    
    /// 初始化存储池
    pub async fn initialize(&mut self) -> Result<()> {
        // 扫描所有磁盘
        let disks = self.disk_monitor.scan_disks().await?;
        
        // 创建存储池
        for disk in disks {
            if disk.is_healthy() {
                let pool = StoragePool::new(disk);
                self.pools.insert(pool.id.clone(), pool);
            }
        }
        
        // 启动健康检查
        self.start_health_check().await?;
        
        Ok(())
    }
    
    /// 选择最佳存储位置
    pub fn select_storage(&self, size: u64) -> Result<PathBuf> {
        // 负载均衡策略：选择空闲空间最多的磁盘
        let pool = self.pools
            .values()
            .filter(|p| p.available_space() > size)
            .max_by_key(|p| p.available_space())
            .ok_or(anyhow!("No available storage"))?;
        
        Ok(pool.base_path.clone())
    }
}
```

---

### 2. 磁盘监控器

```rust
use sysinfo::{System, SystemExt, DiskExt};
use std::time::Duration;

/// 磁盘信息
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: PathBuf,
    pub total_space: u64,
    pub available_space: u64,
    pub disk_type: DiskType,
    pub health_status: HealthStatus,
    pub io_stats: IoStats,
}

#[derive(Debug, Clone)]
pub enum DiskType {
    SSD,
    HDD,
    NVMe,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,       // 健康
    Warning,       // 警告（空间不足、性能下降）
    Critical,      // 严重（即将满、硬件故障）
    Failed,        // 失败
}

/// 磁盘监控器
pub struct DiskMonitor {
    system: System,
}

impl DiskMonitor {
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
        }
    }
    
    /// 扫描所有磁盘
    pub async fn scan_disks(&mut self) -> Result<Vec<DiskInfo>> {
        self.system.refresh_disks_list();
        
        let mut disks = Vec::new();
        
        for disk in self.system.disks() {
            let info = DiskInfo {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_path_buf(),
                total_space: disk.total_space(),
                available_space: disk.available_space(),
                disk_type: Self::detect_disk_type(disk),
                health_status: Self::check_health(disk),
                io_stats: Self::get_io_stats(disk).await?,
            };
            
            disks.push(info);
        }
        
        Ok(disks)
    }
    
    /// 检测磁盘类型
    fn detect_disk_type(disk: &sysinfo::Disk) -> DiskType {
        let name = disk.name().to_string_lossy().to_lowercase();
        
        if name.contains("nvme") {
            DiskType::NVMe
        } else if name.contains("ssd") {
            DiskType::SSD
        } else if name.contains("hd") || name.contains("sd") {
            DiskType::HDD
        } else {
            DiskType::Unknown
        }
    }
    
    /// 检查磁盘健康状态
    fn check_health(disk: &sysinfo::Disk) -> HealthStatus {
        let usage_percent = (disk.total_space() - disk.available_space()) as f64 
                          / disk.total_space() as f64 * 100.0;
        
        if usage_percent >= 95.0 {
            HealthStatus::Critical
        } else if usage_percent >= 85.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }
    
    /// 获取 I/O 统计
    async fn get_io_stats(disk: &sysinfo::Disk) -> Result<IoStats> {
        // 读取 /proc/diskstats (Linux) 或使用系统 API
        #[cfg(target_os = "linux")]
        {
            Self::read_linux_diskstats(disk).await
        }
        
        #[cfg(target_os = "macos")]
        {
            Self::read_macos_iostat(disk).await
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Ok(IoStats::default())
        }
    }
    
    #[cfg(target_os = "linux")]
    async fn read_linux_diskstats(disk: &sysinfo::Disk) -> Result<IoStats> {
        use tokio::fs;
        
        let content = fs::read_to_string("/proc/diskstats").await?;
        
        // 解析 diskstats
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 14 && parts[2].contains(&disk.name().to_string_lossy().to_string()) {
                return Ok(IoStats {
                    read_ops: parts[3].parse().unwrap_or(0),
                    write_ops: parts[7].parse().unwrap_or(0),
                    read_bytes: parts[5].parse::<u64>().unwrap_or(0) * 512,
                    write_bytes: parts[9].parse::<u64>().unwrap_or(0) * 512,
                });
            }
        }
        
        Ok(IoStats::default())
    }
}

/// I/O 统计
#[derive(Debug, Clone, Default)]
pub struct IoStats {
    pub read_ops: u64,
    pub write_ops: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}
```

---

### 3. 健康检查器

```rust
use tokio::time::{interval, Duration};

/// 健康检查器
pub struct HealthChecker {
    check_interval: Duration,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            check_interval: Duration::from_secs(60), // 每分钟检查
        }
    }
    
    /// 启动健康检查任务
    pub async fn start(&self, storage_manager: Arc<RwLock<StorageManager>>) {
        let mut interval = interval(self.check_interval);
        
        loop {
            interval.tick().await;
            
            if let Err(e) = Self::perform_health_check(&storage_manager).await {
                error!("Health check failed: {}", e);
            }
        }
    }
    
    /// 执行健康检查
    async fn perform_health_check(
        storage_manager: &Arc<RwLock<StorageManager>>
    ) -> Result<()> {
        let mut sm = storage_manager.write().await;
        
        // 1. 检查磁盘空间
        Self::check_disk_space(&mut sm).await?;
        
        // 2. 检查 I/O 性能
        Self::check_io_performance(&mut sm).await?;
        
        // 3. 检查数据完整性
        Self::check_data_integrity(&mut sm).await?;
        
        // 4. 检查 SMART 状态（如果支持）
        Self::check_smart_status(&mut sm).await?;
        
        Ok(())
    }
    
    /// 检查磁盘空间
    async fn check_disk_space(sm: &mut StorageManager) -> Result<()> {
        for (id, pool) in &mut sm.pools {
            let usage = pool.usage_percent();
            
            if usage >= 95.0 {
                warn!("Storage pool {} is critically full: {:.1}%", id, usage);
                pool.status = HealthStatus::Critical;
                
                // 触发自动清理
                sm.trigger_cleanup(id).await?;
            } else if usage >= 85.0 {
                warn!("Storage pool {} is running low: {:.1}%", id, usage);
                pool.status = HealthStatus::Warning;
            }
        }
        
        Ok(())
    }
    
    /// 检查 I/O 性能
    async fn check_io_performance(sm: &mut StorageManager) -> Result<()> {
        for (id, pool) in &mut sm.pools {
            let io_stats = pool.get_io_stats().await?;
            
            // 检查 I/O 延迟
            if io_stats.avg_latency_ms > 100.0 {
                warn!("Storage pool {} has high I/O latency: {:.1}ms", 
                      id, io_stats.avg_latency_ms);
            }
        }
        
        Ok(())
    }
    
    /// 检查数据完整性
    async fn check_data_integrity(sm: &mut StorageManager) -> Result<()> {
        // 随机抽样检查文件校验和
        for pool in sm.pools.values() {
            let sample_files = pool.get_sample_files(10).await?;
            
            for file in sample_files {
                if !Self::verify_checksum(&file).await? {
                    error!("Checksum mismatch for file: {:?}", file);
                    // 触发数据修复
                }
            }
        }
        
        Ok(())
    }
    
    /// 检查 SMART 状态
    async fn check_smart_status(sm: &mut StorageManager) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            for pool in sm.pools.values() {
                let output = Command::new("smartctl")
                    .args(&["-H", &pool.device_path])
                    .output()?;
                
                let status = String::from_utf8_lossy(&output.stdout);
                if status.contains("FAILED") {
                    error!("SMART check failed for disk: {}", pool.device_path);
                }
            }
        }
        
        Ok(())
    }
}
```

---

### 4. 智能转码策略

```rust
/// 智能转码决策器
pub struct SmartTranscoder {
    quality_analyzer: QualityAnalyzer,
}

impl SmartTranscoder {
    /// 决定是否需要转码
    pub async fn should_transcode(
        &self,
        input_file: &PathBuf,
        target_quality: &Quality,
    ) -> Result<bool> {
        // 分析输入视频质量
        let input_quality = self.quality_analyzer.analyze(input_file).await?;
        let target_params = target_quality.get_params();
        
        // 比较分辨率
        if input_quality.width < target_params.width 
           || input_quality.height < target_params.height {
            info!(
                "Input resolution {}x{} is lower than target {}x{}, skip transcoding",
                input_quality.width, input_quality.height,
                target_params.width, target_params.height
            );
            return Ok(false);
        }
        
        // 比较码率
        if input_quality.bitrate < target_params.video_bitrate {
            info!(
                "Input bitrate {} is lower than target {}, skip transcoding",
                input_quality.bitrate, target_params.video_bitrate
            );
            return Ok(false);
        }
        
        // 比较帧率
        if input_quality.framerate < target_params.framerate {
            info!(
                "Input framerate {} is lower than target {}, skip transcoding",
                input_quality.framerate, target_params.framerate
            );
            return Ok(false);
        }
        
        // 需要转码
        Ok(true)
    }
    
    /// 智能转码（自动调整参数）
    pub async fn smart_transcode(
        &self,
        input_file: &PathBuf,
        output_file: &PathBuf,
        target_quality: &Quality,
    ) -> Result<()> {
        let input_quality = self.quality_analyzer.analyze(input_file).await?;
        let mut target_params = target_quality.get_params();
        
        // 调整目标参数，不超过输入质量
        target_params.width = target_params.width.min(input_quality.width);
        target_params.height = target_params.height.min(input_quality.height);
        target_params.video_bitrate = target_params.video_bitrate.min(input_quality.bitrate);
        target_params.framerate = target_params.framerate.min(input_quality.framerate);
        
        // 执行转码
        self.transcode_with_params(input_file, output_file, &target_params).await
    }
}

/// 视频质量分析器
pub struct QualityAnalyzer;

impl QualityAnalyzer {
    /// 分析视频质量
    pub async fn analyze(&self, file: &PathBuf) -> Result<VideoQuality> {
        use std::process::Command;
        
        // 使用 ffprobe 分析视频
        let output = Command::new("ffprobe")
            .args(&[
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "stream=width,height,bit_rate,r_frame_rate",
                "-of", "json",
                file.to_str().unwrap(),
            ])
            .output()?;
        
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        
        let stream = &json["streams"][0];
        
        Ok(VideoQuality {
            width: stream["width"].as_u64().unwrap_or(0) as u32,
            height: stream["height"].as_u64().unwrap_or(0) as u32,
            bitrate: stream["bit_rate"].as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            framerate: Self::parse_framerate(stream["r_frame_rate"].as_str().unwrap_or("25/1")),
        })
    }
    
    fn parse_framerate(fps_str: &str) -> u32 {
        let parts: Vec<&str> = fps_str.split('/').collect();
        if parts.len() == 2 {
            let num: f64 = parts[0].parse().unwrap_or(25.0);
            let den: f64 = parts[1].parse().unwrap_or(1.0);
            (num / den) as u32
        } else {
            25
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoQuality {
    pub width: u32,
    pub height: u32,
    pub bitrate: u64,
    pub framerate: u32,
}
```

---

## 📊 存储池管理

### 存储池结构

```rust
/// 存储池
pub struct StoragePool {
    pub id: String,
    pub base_path: PathBuf,
    pub device_path: String,
    pub disk_type: DiskType,
    pub total_space: u64,
    pub available_space: u64,
    pub status: HealthStatus,
    pub io_stats: IoStats,
}

impl StoragePool {
    /// 使用率
    pub fn usage_percent(&self) -> f64 {
        (self.total_space - self.available_space) as f64 
        / self.total_space as f64 * 100.0
    }
    
    /// 可用空间
    pub fn available_space(&self) -> u64 {
        self.available_space
    }
    
    /// 是否健康
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }
}
```

---

## 🎯 配置示例

```toml
# config/storage.toml

[storage]
# 存储池配置
[[storage.pools]]
name = "ssd-pool"
path = "/mnt/ssd/recordings"
type = "ssd"
priority = 1                    # 优先级（实时录像）

[[storage.pools]]
name = "hdd-pool-1"
path = "/mnt/hdd1/recordings"
type = "hdd"
priority = 2                    # 归档存储

[[storage.pools]]
name = "hdd-pool-2"
path = "/mnt/hdd2/recordings"
type = "hdd"
priority = 2

# 健康检查配置
[storage.health_check]
enabled = true
interval_seconds = 60           # 每分钟检查
check_smart = true              # 检查 SMART 状态
check_io_performance = true     # 检查 I/O 性能

# 告警配置
[storage.alerts]
space_warning_percent = 85      # 空间警告阈值
space_critical_percent = 95     # 空间严重阈值
io_latency_warning_ms = 100     # I/O 延迟警告

# 自动清理配置
[storage.auto_cleanup]
enabled = true
trigger_at_percent = 90         # 90% 时触发清理
target_percent = 80             # 清理到 80%

# 智能转码配置
[storage.smart_transcode]
enabled = true
skip_if_lower_quality = true    # 原始画质低于目标则不转码
auto_adjust_params = true       # 自动调整转码参数
```

---

## 📈 监控指标

### 存储指标

```rust
pub struct StorageMetrics {
    /// 总空间
    pub total_space: u64,
    
    /// 已用空间
    pub used_space: u64,
    
    /// 可用空间
    pub available_space: u64,
    
    /// I/O 统计
    pub read_ops_per_sec: f64,
    pub write_ops_per_sec: f64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    
    /// 平均延迟
    pub avg_read_latency_ms: f64,
    pub avg_write_latency_ms: f64,
    
    /// 健康状态
    pub healthy_disks: usize,
    pub warning_disks: usize,
    pub critical_disks: usize,
}
```

---

## 🎯 总结

**增强功能**：
1. ✅ **磁盘监控** - 实时检测磁盘健康状态
2. ✅ **存储池管理** - 多磁盘负载均衡
3. ✅ **智能转码** - 原始画质低于目标则不转码
4. ✅ **健康检查** - 定期检查空间、I/O、SMART
5. ✅ **自动清理** - 空间不足时自动清理
6. ✅ **性能监控** - I/O 统计和告警

**智能转码策略**：
```rust
if input_quality < target_quality {
    // 不转码，保持原始质量
    skip_transcode();
} else {
    // 转码到目标质量
    transcode();
}
```

参考 MinIO 的企业级功能，打造健壮的存储系统！🚀

---

**文档完成时间**: 2026-02-19 19:10 UTC+08:00  
**状态**: ✅ **增强设计完成**
