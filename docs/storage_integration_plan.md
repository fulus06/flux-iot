# 存储模块与协议集成方案

**设计时间**: 2026-02-19 20:00 UTC+08:00  
**状态**: 📋 **集成规划**

---

## 🎯 集成目标

将 **flux-storage** 和 **flux-notify** 模块集成到：
1. ✅ 时移系统（TimeShiftCore）
2. ✅ RTMP 服务（flux-rtmpd）
3. ✅ RTSP 服务（flux-rtspd）
4. ✅ SRT 服务（flux-srt）
5. ✅ GB28181 服务（flux-gb28181d）

---

## 🏗️ 集成架构

```
┌─────────────────────────────────────────────────────────┐
│                  协议层                                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │  RTMP    │ │  RTSP    │ │   SRT    │ │ GB28181  │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘  │
└───────┼────────────┼────────────┼────────────┼─────────┘
        │            │            │            │
        └────────────┴────────────┴────────────┘
                     ↓
┌─────────────────────────────────────────────────────────┐
│              flux-media-core（媒体核心）                 │
│  ┌──────────────────────────────────────────────┐      │
│  │  TimeShiftCore（时移核心）                    │      │
│  │  - 热缓存（内存）                             │      │
│  │  - 冷索引（磁盘）                             │      │
│  └──────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────┐
│              存储层（flux-storage）                      │
│  ┌──────────────────────────────────────────────┐      │
│  │  StorageManager（存储管理器）                 │      │
│  │  - 磁盘监控                                   │      │
│  │  - 存储池管理                                 │      │
│  │  - 负载均衡                                   │      │
│  │  - 健康检查                                   │      │
│  └──────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────┐
│              通知层（flux-notify）                       │
│  ┌──────────────────────────────────────────────┐      │
│  │  NotifyManager（通知管理器）                  │      │
│  │  - 磁盘空间告警                               │      │
│  │  - 系统错误通知                               │      │
│  │  - 多渠道发送                                 │      │
│  └──────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────┘
```

---

## 📦 集成步骤

### 1. flux-media-core 集成存储管理器

#### 更新 TimeShiftCore

```rust
// crates/flux-media-core/src/timeshift/core.rs

use flux_storage::{StorageManager, PoolConfig};
use std::sync::Arc;

pub struct TimeShiftCore {
    // 现有字段
    hot_cache: Arc<RwLock<HotCache>>,
    cold_index: Arc<RwLock<ColdIndex>>,
    
    // 新增：存储管理器
    storage_manager: Arc<StorageManager>,
}

impl TimeShiftCore {
    pub async fn new(config: TimeShiftConfig, storage_manager: Arc<StorageManager>) -> Result<Self> {
        Ok(Self {
            hot_cache: Arc::new(RwLock::new(HotCache::new(config.hot_cache_size))),
            cold_index: Arc::new(RwLock::new(ColdIndex::new())),
            storage_manager,
        })
    }
    
    /// 添加分片时使用存储管理器选择路径
    pub async fn add_segment(&self, segment: Segment) -> Result<()> {
        // 使用存储管理器选择最佳存储位置
        let storage_path = self.storage_manager
            .select_pool(segment.size as u64)
            .await?;
        
        // 保存到选定的存储池
        let file_path = storage_path.join(&segment.filename);
        tokio::fs::write(&file_path, &segment.data).await?;
        
        // 更新索引
        let mut cold_index = self.cold_index.write().await;
        cold_index.add_segment(segment.stream_id, segment.timestamp, file_path);
        
        Ok(())
    }
}
```

---

### 2. RTMP 服务集成

#### 更新 main.rs

```rust
// crates/flux-rtmpd/src/main.rs

use flux_storage::{StorageManager, PoolConfig, DiskType};
use flux_notify::{NotifyManager, NotifyLevel, NotifyChannel, NotifyMessage};
use flux_notify::{EmailNotifier, EmailConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 创建存储管理器
    let storage_manager = Arc::new(StorageManager::new());
    
    // 配置存储池
    let storage_configs = vec![
        PoolConfig {
            name: "ssd-realtime".to_string(),
            path: PathBuf::from("/mnt/ssd/rtmp"),
            disk_type: DiskType::SSD,
            priority: 1,
            max_usage_percent: 90.0,
        },
        PoolConfig {
            name: "hdd-archive".to_string(),
            path: PathBuf::from("/mnt/hdd/rtmp"),
            disk_type: DiskType::HDD,
            priority: 2,
            max_usage_percent: 95.0,
        },
    ];
    
    storage_manager.initialize(storage_configs).await?;
    
    // 启动健康检查
    storage_manager.clone().start_health_check_task().await;
    
    // 2. 创建通知管理器
    let notify_manager = Arc::new(NotifyManager::new(NotifyLevel::Warning));
    
    // 注册邮件通知
    let email_notifier = EmailNotifier::new(EmailConfig {
        smtp_host: "smtp.example.com".to_string(),
        smtp_port: 587,
        username: "noreply@flux-iot.com".to_string(),
        password: "password".to_string(),
        from: "noreply@flux-iot.com".to_string(),
        to: vec!["admin@example.com".to_string()],
    });
    notify_manager.register(NotifyChannel::Email, Box::new(email_notifier)).await;
    
    // 3. 创建时移核心（传入存储管理器）
    let timeshift = Arc::new(
        TimeShiftCore::new(timeshift_config, storage_manager.clone()).await?
    );
    
    // 4. 启动存储监控任务
    let storage_clone = storage_manager.clone();
    let notify_clone = notify_manager.clone();
    tokio::spawn(async move {
        monitor_storage(storage_clone, notify_clone).await;
    });
    
    // 5. 创建 HLS 管理器
    let hls_manager = Arc::new(
        HlsManager::new(hls_config, Some(timeshift.clone())).await?
    );
    
    // ... 其余代码
}

/// 存储监控任务
async fn monitor_storage(
    storage: Arc<StorageManager>,
    notify: Arc<NotifyManager>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        
        // 刷新存储状态
        if let Err(e) = storage.refresh().await {
            error!("Storage refresh failed: {}", e);
            continue;
        }
        
        // 检查存储池状态
        let pools = storage.get_pools().await;
        for (name, path, usage, status) in pools {
            use flux_storage::HealthStatus;
            
            match status {
                HealthStatus::Warning => {
                    let message = NotifyMessage::warning(
                        format!("RTMP 存储池 {} 空间警告", name),
                        format!("路径: {:?}\n使用率: {:.1}%\n建议清理旧文件", path, usage)
                    );
                    let _ = notify.broadcast(&message).await;
                }
                HealthStatus::Critical => {
                    let message = NotifyMessage::critical(
                        format!("RTMP 存储池 {} 空间严重不足", name),
                        format!("路径: {:?}\n使用率: {:.1}%\n请立即处理！", path, usage)
                    );
                    let _ = notify.broadcast(&message).await;
                }
                HealthStatus::Failed => {
                    let message = NotifyMessage::critical(
                        format!("RTMP 存储池 {} 故障", name),
                        format!("路径: {:?}\n磁盘可能已损坏", path)
                    );
                    let _ = notify.broadcast(&message).await;
                }
                _ => {}
            }
        }
    }
}
```

---

### 3. 统一配置文件

#### config/storage.toml

```toml
# 存储配置

[storage]
# 健康检查
[storage.health_check]
enabled = true
interval_seconds = 60
warning_threshold = 85.0
critical_threshold = 95.0

# 自动清理
[storage.auto_cleanup]
enabled = true
trigger_at_percent = 90.0
target_percent = 80.0

# RTMP 存储池
[[storage.rtmp.pools]]
name = "ssd-realtime"
path = "/mnt/ssd/rtmp"
type = "ssd"
priority = 1
max_usage_percent = 90.0

[[storage.rtmp.pools]]
name = "hdd-archive"
path = "/mnt/hdd/rtmp"
type = "hdd"
priority = 2
max_usage_percent = 95.0

# RTSP 存储池
[[storage.rtsp.pools]]
name = "ssd-realtime"
path = "/mnt/ssd/rtsp"
type = "ssd"
priority = 1
max_usage_percent = 90.0

[[storage.rtsp.pools]]
name = "hdd-archive"
path = "/mnt/hdd/rtsp"
type = "hdd"
priority = 2
max_usage_percent = 95.0

# SRT 存储池
[[storage.srt.pools]]
name = "ssd-realtime"
path = "/mnt/ssd/srt"
type = "ssd"
priority = 1
max_usage_percent = 90.0

# GB28181 存储池
[[storage.gb28181.pools]]
name = "ssd-realtime"
path = "/mnt/ssd/gb28181"
type = "ssd"
priority = 1
max_usage_percent = 90.0
```

#### config/notify.toml

```toml
# 通知配置

[notify]
# 最小通知级别
min_level = "warning"  # info/warning/error/critical

# 邮件通知
[notify.email]
enabled = true
smtp_host = "smtp.example.com"
smtp_port = 587
username = "noreply@flux-iot.com"
password = "your-password"
from = "noreply@flux-iot.com"
to = ["admin@example.com"]

# 钉钉通知
[notify.dingtalk]
enabled = true
webhook_url = "https://oapi.dingtalk.com/robot/send?access_token=xxx"

# 企业微信通知
[notify.wechat]
enabled = false
webhook_url = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxx"

# Webhook 通知
[notify.webhook]
enabled = false
url = "https://api.example.com/webhook"
method = "POST"
```

---

## 🔄 数据流

### 录像流程

```
1. 协议接收流
   ↓
2. 编码/转码
   ↓
3. 分片（HLS/DASH）
   ↓
4. StorageManager.select_pool() ← 选择最佳存储位置
   ↓
5. 写入磁盘
   ↓
6. TimeShiftCore.add_segment() ← 添加到时移索引
   ↓
7. 健康检查 → 如果空间不足 → NotifyManager.broadcast()
```

### 时移播放流程

```
1. 客户端请求历史流（start_time）
   ↓
2. TimeShiftCore.get_segments(start_time)
   ↓
3. 从 ColdIndex 查询分片位置
   ↓
4. 从磁盘读取分片
   ↓
5. 返回 M3U8 播放列表
```

---

## 📊 集成效果

### 存储管理

| 功能 | 集成前 | 集成后 |
|------|--------|--------|
| **存储位置** | 硬编码路径 | ✅ 动态负载均衡 |
| **磁盘监控** | ❌ 无 | ✅ 实时监控 |
| **空间告警** | ❌ 无 | ✅ 多渠道通知 |
| **健康检查** | ❌ 无 | ✅ 自动检查 |
| **多磁盘** | ❌ 单磁盘 | ✅ 多磁盘池 |

### 通知系统

| 场景 | 通知方式 |
|------|---------|
| **磁盘空间 > 85%** | Warning 级别通知 |
| **磁盘空间 > 95%** | Critical 级别通知 |
| **磁盘故障** | Critical 级别通知 |
| **系统错误** | Error 级别通知 |

---

## 🎯 集成优势

1. **智能存储**
   - ✅ 自动选择最佳磁盘
   - ✅ SSD 用于实时，HDD 用于归档
   - ✅ 负载均衡

2. **主动监控**
   - ✅ 实时磁盘健康检查
   - ✅ 空间使用监控
   - ✅ 自动告警

3. **多渠道通知**
   - ✅ 邮件
   - ✅ 钉钉
   - ✅ 企业微信
   - ✅ Webhook

4. **统一管理**
   - ✅ 所有协议共享存储管理器
   - ✅ 统一配置
   - ✅ 统一监控

---

## 📝 实施计划

### 第一阶段：核心集成
1. ✅ 在 flux-media-core 中集成 StorageManager
2. ✅ 更新 TimeShiftCore 使用存储管理器
3. ✅ 添加存储监控任务

### 第二阶段：协议集成
1. ✅ RTMP 服务集成
2. ✅ RTSP 服务集成
3. ✅ SRT 服务集成
4. ✅ GB28181 服务集成

### 第三阶段：配置和测试
1. ✅ 创建统一配置文件
2. ✅ 集成测试
3. ✅ 性能测试
4. ✅ 文档完善

---

**设计完成时间**: 2026-02-19 20:00 UTC+08:00  
**状态**: ✅ **集成方案完成**
