use crate::health::HealthStatus;
use crate::manager::{HealthCheckTaskHandle, StorageManager};
use crate::pool::PoolConfig;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use flux_notify::{NotifyManager, NotifyMessage};
use crate::metrics::StorageMetrics;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration as StdDuration, SystemTime};
use tokio::sync::RwLock;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{error, info, warn};

/// 存储监控服务
pub struct MonitorService {
    /// 存储管理器
    storage_manager: Arc<StorageManager>,
    
    /// 通知管理器
    notify_manager: Arc<NotifyManager>,
    
    /// 上次告警时间（用于去重）
    last_alert_time: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,

    /// 上次健康状态（用于状态变化与恢复判断）
    last_status: Arc<RwLock<HashMap<String, HealthStatus>>>,

    /// 是否发送恢复通知
    send_recovery_alerts: bool,

    /// 是否仅在状态变化时发送告警
    alert_on_status_change: bool,
    
    /// 监控间隔
    check_interval_secs: u64,
    
    /// 告警去重间隔
    alert_dedup_duration: Duration,

    enable_lifecycle_gc: bool,
    retention_days: u64,
    max_capacity_gb: Option<u64>,
    gc_trigger_usage_percent: f64,
    gc_target_usage_percent: f64,

    notify_gc_results: bool,

    telemetry_enabled: bool,
    telemetry_endpoint: Option<String>,
    telemetry_timeout: StdDuration,
    telemetry_client: reqwest::Client,
}

struct FileEntry {
    path: PathBuf,
    modified: SystemTime,
    size: u64,
}

pub struct MonitorTaskHandle {
    shutdown_tx: watch::Sender<bool>,
    join_handle: JoinHandle<()>,
}

impl MonitorTaskHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.join_handle.await;
    }

    pub fn abort(self) {
        self.join_handle.abort();
    }
}

impl MonitorService {
    /// 创建监控服务
    pub async fn new(
        storage_configs: Vec<PoolConfig>,
        notify_manager: Arc<NotifyManager>,
        check_interval_secs: u64,
        alert_dedup_minutes: i64,
        send_recovery_alerts: bool,
        alert_on_status_change: bool,
        enable_lifecycle_gc: bool,
        retention_days: u64,
        max_capacity_gb: Option<u64>,
        gc_trigger_usage_percent: f64,
        gc_target_usage_percent: f64,
        notify_gc_results: bool,
        telemetry_enabled: bool,
        telemetry_endpoint: Option<String>,
        telemetry_timeout_ms: u64,
    ) -> Result<Self> {
        info!("Initializing StorageMonitorService");
        
        // 创建存储管理器
        let storage_manager = Arc::new(StorageManager::new());
        storage_manager.initialize(storage_configs).await?;
        
        // 注意：不在这里启动健康检查任务，由调用者决定是否启动
        // 避免在测试环境中创建无法停止的后台任务
        
        Ok(Self {
            storage_manager,
            notify_manager,
            last_alert_time: Arc::new(RwLock::new(HashMap::new())),
            last_status: Arc::new(RwLock::new(HashMap::new())),
            send_recovery_alerts,
            alert_on_status_change,
            check_interval_secs,
            alert_dedup_duration: Duration::minutes(alert_dedup_minutes),
            enable_lifecycle_gc,
            retention_days,
            max_capacity_gb,
            gc_trigger_usage_percent,
            gc_target_usage_percent,
            notify_gc_results,
            telemetry_enabled,
            telemetry_endpoint,
            telemetry_timeout: StdDuration::from_millis(telemetry_timeout_ms),
            telemetry_client: reqwest::Client::new(),
        })
    }
    
    /// 启动监控任务
    pub async fn start_monitoring(self: Arc<Self>) {
        info!("Starting storage monitoring task");
        
        let mut interval = interval(tokio::time::Duration::from_secs(self.check_interval_secs));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.check_and_alert().await {
                error!("Monitoring check failed: {}", e);
            }
        }
    }

    pub fn start_monitoring_task(self: Arc<Self>) -> MonitorTaskHandle {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let join_handle = tokio::spawn(async move {
            let mut interval = interval(tokio::time::Duration::from_secs(self.check_interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = self.check_and_alert().await {
                            error!("Monitoring check failed: {}", e);
                        }

                        if self.enable_lifecycle_gc {
                            if let Err(e) = self.run_lifecycle_gc().await {
                                warn!("Lifecycle GC failed: {}", e);
                            }
                        }
                    }
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        MonitorTaskHandle {
            shutdown_tx,
            join_handle,
        }
    }

    async fn run_lifecycle_gc(&self) -> Result<()> {
        let pools = self.storage_manager.get_pools_stats().await;
        let now = Utc::now();
        let cutoff = now - Duration::days(self.retention_days as i64);

        let mut total_deleted_files: u64 = 0;
        let mut total_freed_bytes: u64 = 0;
        let mut lines: Vec<String> = Vec::new();

        for pool in pools {
            let pool_name = pool.name.clone();
            let root = pool.path;

            let retention_cutoff = if self.retention_days == 0 {
                None
            } else {
                Some(cutoff)
            };

            let cap_bytes = self
                .max_capacity_gb
                .and_then(|gb| gb.checked_mul(1024 * 1024 * 1024));

            let trigger_by_usage = pool.usage_percent >= self.gc_trigger_usage_percent;

            let (mut entries, mut dir_size) = if cap_bytes.is_some() || retention_cutoff.is_some() {
                self.scan_files_and_size(&root).await?
            } else {
                (Vec::new(), 0u64)
            };

            let before_size = dir_size;
            let mut pool_deleted_files: u64 = 0;
            let mut pool_freed_bytes: u64 = 0;
            let mut reasons: Vec<&'static str> = Vec::new();

            if let Some(cutoff) = retention_cutoff {
                let before = cutoff;
                let (deleted_files, freed_bytes) = self
                    .delete_by_retention(&root, &mut entries, before)
                    .await?;
                if deleted_files > 0 {
                    let (_entries2, size2) = self.scan_files_and_size(&root).await?;
                    dir_size = size2;
                    entries = _entries2;
                    pool_deleted_files = pool_deleted_files.saturating_add(deleted_files as u64);
                    pool_freed_bytes = pool_freed_bytes.saturating_add(freed_bytes);
                    reasons.push("retention");
                }
            }

            let mut should_gc_by_cap = false;
            let mut target_bytes = None;

            if let Some(cap) = cap_bytes {
                if dir_size > cap {
                    should_gc_by_cap = true;
                }

                if (dir_size as f64) / (cap as f64) * 100.0 >= self.gc_trigger_usage_percent {
                    should_gc_by_cap = true;
                }

                let target = (cap as f64 * (self.gc_target_usage_percent / 100.0)) as u64;
                target_bytes = Some(target);
            }

            if trigger_by_usage || should_gc_by_cap {
                let desired_size = target_bytes.unwrap_or(dir_size);
                if desired_size < dir_size {
                    let (deleted_files, freed_bytes) = self
                        .delete_oldest_until_size(&root, &mut entries, desired_size)
                        .await?;
                    if deleted_files > 0 {
                        pool_deleted_files = pool_deleted_files.saturating_add(deleted_files as u64);
                        pool_freed_bytes = pool_freed_bytes.saturating_add(freed_bytes);
                        reasons.push("capacity");
                    }
                }
            }

            if pool_deleted_files > 0 {
                total_deleted_files = total_deleted_files.saturating_add(pool_deleted_files);
                total_freed_bytes = total_freed_bytes.saturating_add(pool_freed_bytes);
                let after_size = before_size.saturating_sub(pool_freed_bytes);
                lines.push(format!(
                    "pool={} path={:?} deleted_files={} freed={} before={} after={} reasons={}",
                    pool_name,
                    root,
                    pool_deleted_files,
                    StorageMetrics::format_space(pool_freed_bytes),
                    StorageMetrics::format_space(before_size),
                    StorageMetrics::format_space(after_size),
                    reasons.join(",")
                ));
            }
        }

        if self.notify_gc_results && total_deleted_files > 0 {
            let title = "存储生命周期 GC 执行结果".to_string();
            let mut content = format!(
                "deleted_files={} freed={}\n",
                total_deleted_files,
                StorageMetrics::format_space(total_freed_bytes)
            );
            for l in &lines {
                content.push_str(&l);
                content.push('\n');
            }

            let message = NotifyMessage::new(title, content, flux_notify::NotifyLevel::Info);
            if let Err(e) = self.notify_manager.broadcast(&message).await {
                warn!("Failed to send GC result notification: {}", e);
            }
        }

        if self.telemetry_enabled {
            if let Some(endpoint) = self.telemetry_endpoint.as_ref() {
                if total_deleted_files > 0 {
                    let payload = json!({
                        "service": "flux-storage-monitor",
                        "event": "gc",
                        "deleted_files": total_deleted_files,
                        "freed_bytes": total_freed_bytes,
                        "details": lines,
                    });
                    self.post_telemetry(endpoint, "storage/gc", payload).await;
                }
            }
        }

        Ok(())
    }

    async fn post_telemetry(&self, endpoint: &str, topic: &str, payload: serde_json::Value) {
        if endpoint.is_empty() {
            return;
        }

        let req_body = json!({
            "topic": topic,
            "payload": payload,
        });

        let result = self
            .telemetry_client
            .post(endpoint)
            .timeout(self.telemetry_timeout)
            .json(&req_body)
            .send()
            .await;

        match result {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!("Telemetry post failed: status={} topic={} endpoint={}", resp.status(), topic, endpoint);
                }
            }
            Err(e) => {
                warn!("Telemetry post error: {} topic={} endpoint={}", e, topic, endpoint);
            }
        }
    }

    async fn scan_files_and_size(&self, root: &PathBuf) -> Result<(Vec<FileEntry>, u64)> {
        let root = root.clone();
        tokio::task::spawn_blocking(move || {
            fn walk(dir: &PathBuf, entries: &mut Vec<FileEntry>, total: &mut u64) -> Result<()> {
                let rd = std::fs::read_dir(dir)?;
                for item in rd {
                    let item = item?;
                    let path = item.path();
                    let meta = item.metadata()?;

                    if meta.file_type().is_symlink() {
                        continue;
                    }

                    if meta.is_dir() {
                        walk(&path, entries, total)?;
                        continue;
                    }

                    if !meta.is_file() {
                        continue;
                    }

                    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    let size = meta.len();
                    *total = total.saturating_add(size);
                    entries.push(FileEntry {
                        path,
                        modified,
                        size,
                    });
                }
                Ok(())
            }

            let mut entries = Vec::new();
            let mut total = 0u64;
            if root.exists() {
                let _ = walk(&root, &mut entries, &mut total);
            }
            Ok((entries, total))
        })
        .await?
    }

    async fn delete_by_retention(
        &self,
        root: &PathBuf,
        entries: &mut Vec<FileEntry>,
        cutoff: DateTime<Utc>,
    ) -> Result<(usize, u64)> {
        let cutoff_st = SystemTime::from(cutoff);

        let mut to_delete = Vec::new();
        entries.retain(|e| {
            if e.modified < cutoff_st {
                to_delete.push((e.path.clone(), e.size));
                false
            } else {
                true
            }
        });

        if to_delete.is_empty() {
            return Ok((0, 0));
        }

        let root = root.clone();
        let deleted = tokio::task::spawn_blocking(move || {
            let mut count = 0usize;
            let mut freed = 0u64;
            for p in to_delete {
                if !p.0.starts_with(&root) {
                    continue;
                }
                if std::fs::remove_file(&p.0).is_ok() {
                    count += 1;
                    freed = freed.saturating_add(p.1);
                }
            }
            (count, freed)
        })
        .await?;

        Ok(deleted)
    }

    async fn delete_oldest_until_size(
        &self,
        root: &PathBuf,
        entries: &mut Vec<FileEntry>,
        target_size: u64,
    ) -> Result<(usize, u64)> {
        entries.sort_by_key(|e| e.modified);

        let mut current_size = entries.iter().map(|e| e.size).sum::<u64>();
        if current_size <= target_size {
            return Ok((0, 0));
        }

        let mut to_delete: Vec<(PathBuf, u64)> = Vec::new();
        for e in entries.iter() {
            if current_size <= target_size {
                break;
            }
            current_size = current_size.saturating_sub(e.size);
            to_delete.push((e.path.clone(), e.size));
        }

        if to_delete.is_empty() {
            return Ok((0, 0));
        }

        let root = root.clone();
        let deleted = tokio::task::spawn_blocking(move || {
            let mut count = 0usize;
            let mut freed = 0u64;
            for p in to_delete {
                if !p.0.starts_with(&root) {
                    continue;
                }
                if std::fs::remove_file(&p.0).is_ok() {
                    count += 1;
                    freed = freed.saturating_add(p.1);
                }
            }
            (count, freed)
        })
        .await?;

        tokio::time::sleep(StdDuration::from_millis(10)).await;

        Ok(deleted)
    }
    
    /// 检查存储状态并发送告警
    async fn check_and_alert(&self) -> Result<()> {
        // 获取所有存储池状态
        let pools = self.storage_manager.get_pools().await;
        let mut last_alert = self.last_alert_time.write().await;
        let mut last_status = self.last_status.write().await;
        let now = Utc::now();
        
        for (name, path, usage, status) in pools {
            let prev_status = last_status.get(&name).copied();

            // 恢复通知：从告警态恢复为 Healthy
            if self.send_recovery_alerts {
                if let Some(prev) = prev_status {
                    if prev.needs_alert() && status == HealthStatus::Healthy {
                        let alert_key = format!("{}:recovered", name);
                        if let Some(last_time) = last_alert.get(&alert_key) {
                            if now - *last_time < self.alert_dedup_duration {
                                last_status.insert(name.clone(), status);
                                continue;
                            }
                        }

                        let message = self.create_recovery_message(&name, &path, usage, prev);
                        match self.notify_manager.broadcast(&message).await {
                            Ok(_) => {
                                info!("Recovery alert sent for storage pool: {}", name);
                                last_alert.insert(alert_key, now);
                            }
                            Err(e) => {
                                warn!("Failed to send recovery alert for {}: {}", name, e);
                            }
                        }

                        if self.telemetry_enabled {
                            if let Some(endpoint) = self.telemetry_endpoint.as_ref() {
                                let payload = json!({
                                    "service": "flux-storage-monitor",
                                    "event": "recovery",
                                    "pool": name,
                                    "path": path,
                                    "usage_percent": usage,
                                    "prev_status": format!("{:?}", prev),
                                    "status": "Healthy",
                                });
                                self.post_telemetry(endpoint, "storage/recovery", payload).await;
                            }
                        }
                    }
                }
            }

            // 更新状态缓存
            last_status.insert(name.clone(), status);

            // 只处理需要告警的状态
            if !status.needs_alert() {
                continue;
            }

            if self.alert_on_status_change {
                if let Some(prev) = prev_status {
                    if prev == status {
                        continue;
                    }
                }
            }
            
            // 生成告警键（用于去重）
            let alert_key = format!("{}:{:?}", name, status);
            
            // 检查是否需要去重
            if let Some(last_time) = last_alert.get(&alert_key) {
                if now - *last_time < self.alert_dedup_duration {
                    continue; // 跳过重复告警
                }
            }
            
            // 发送告警
            let message = self.create_alert_message(&name, &path, usage, status);
            
            match self.notify_manager.broadcast(&message).await {
                Ok(_) => {
                    info!("Alert sent for storage pool: {}", name);
                    last_alert.insert(alert_key, now);
                }
                Err(e) => {
                    warn!("Failed to send alert for {}: {}", name, e);
                }
            }

            if self.telemetry_enabled {
                if let Some(endpoint) = self.telemetry_endpoint.as_ref() {
                    let payload = json!({
                        "service": "flux-storage-monitor",
                        "event": "alert",
                        "pool": name,
                        "path": path,
                        "usage_percent": usage,
                        "status": format!("{:?}", status),
                    });
                    self.post_telemetry(endpoint, "storage/alert", payload).await;
                }
            }
        }
        
        Ok(())
    }
    
    /// 创建告警消息
    fn create_alert_message(
        &self,
        name: &str,
        path: &std::path::PathBuf,
        usage: f64,
        status: HealthStatus,
    ) -> NotifyMessage {
        let (title, level) = match status {
            HealthStatus::Warning => (
                format!("存储池 {} 空间警告", name),
                flux_notify::NotifyLevel::Warning,
            ),
            HealthStatus::Critical => (
                format!("存储池 {} 空间严重不足", name),
                flux_notify::NotifyLevel::Critical,
            ),
            HealthStatus::Failed => (
                format!("存储池 {} 故障", name),
                flux_notify::NotifyLevel::Critical,
            ),
            _ => (
                format!("存储池 {} 状态变化", name),
                flux_notify::NotifyLevel::Info,
            ),
        };
        
        let content = format!(
            "存储池: {}\n路径: {:?}\n使用率: {:.1}%\n状态: {:?}\n\n请及时处理！",
            name, path, usage, status
        );
        
        NotifyMessage::new(title, content, level)
    }

    /// 创建恢复消息
    fn create_recovery_message(
        &self,
        name: &str,
        path: &std::path::PathBuf,
        usage: f64,
        prev_status: HealthStatus,
    ) -> NotifyMessage {
        let title = format!("存储池 {} 已恢复", name);
        let content = format!(
            "存储池: {}\n路径: {:?}\n使用率: {:.1}%\n恢复前状态: {:?}\n当前状态: {:?}",
            name,
            path,
            usage,
            prev_status,
            HealthStatus::Healthy
        );
        NotifyMessage::new(title, content, flux_notify::NotifyLevel::Info)
    }
    
    /// 获取存储管理器（用于外部查询）
    pub fn storage_manager(&self) -> &Arc<StorageManager> {
        &self.storage_manager
    }
    
    /// 获取存储指标
    pub async fn get_metrics(&self) -> crate::metrics::StorageMetrics {
        self.storage_manager.get_metrics().await
    }
    
    /// 启动存储管理器的健康检查任务
    pub fn start_storage_health_check(self: &Arc<Self>) -> HealthCheckTaskHandle {
        self.storage_manager.clone().start_health_check_task_handle()
    }
}
