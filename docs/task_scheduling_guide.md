# 任务调度指南 - FLUX TimeSeries

> **版本**: v1.0.0  
> **日期**: 2026-02-22

---

## 📋 概述

FLUX TimeSeries 提供了基于 Cron 表达式的任务调度功能，支持自动化的归档、清理和降采样任务。

---

## 🎯 支持的任务类型

### 1. 归档任务（Archive）

自动归档历史数据到文件或对象存储。

```rust
let policy = ArchivePolicy {
    table_name: "device_metrics".to_string(),
    archive_older_than: Duration::days(365),
    destination: ArchiveDestination::LocalFile {
        path: "/archive/".to_string(),
    },
    delete_after_archive: false,
};

let task = ScheduledTask::daily_archive(policy);
```

### 2. 清理任务（Cleanup）

自动清理过期数据，释放存储空间。

```rust
let policy = CleanupPolicy::for_metrics();
let task = ScheduledTask::daily_cleanup(policy);
```

### 3. 降采样刷新任务（Downsample Refresh）

自动刷新降采样视图。

```rust
let task = ScheduledTask::hourly_downsample_refresh("device_metrics_1h".to_string());
```

---

## ⏰ 预定义调度任务

### 每日归档

```rust
let task = ScheduledTask::daily_archive(policy);
// Cron: 0 0 2 * * * (每天凌晨 2 点)
```

### 每周归档

```rust
let task = ScheduledTask::weekly_archive(policy);
// Cron: 0 0 3 * * 0 (每周日凌晨 3 点)
```

### 每月归档

```rust
let task = ScheduledTask::monthly_archive(policy);
// Cron: 0 0 4 1 * * (每月 1 号凌晨 4 点)
```

### 每日清理

```rust
let task = ScheduledTask::daily_cleanup(policy);
// Cron: 0 0 1 * * * (每天凌晨 1 点)
```

### 每小时刷新

```rust
let task = ScheduledTask::hourly_downsample_refresh(view_name);
// Cron: 0 0 * * * * (每小时整点)
```

---

## 🔧 使用方法

### 1. 创建调度器

```rust
use flux_timeseries::{TaskScheduler, TimescaleStore};

let store = TimescaleStore::new(database_url).await?;
let db = store.connection();

let scheduler = TaskScheduler::new(db.clone().into()).await?;
```

### 2. 添加任务

```rust
// 添加每日归档任务
let archive_policy = ArchivePolicy { /* ... */ };
let task = ScheduledTask::daily_archive(archive_policy);
let job_id = scheduler.add_task(task).await?;

// 添加每日清理任务
let cleanup_policy = CleanupPolicy::for_metrics();
let task = ScheduledTask::daily_cleanup(cleanup_policy);
let job_id = scheduler.add_task(task).await?;
```

### 3. 启动调度器

```rust
scheduler.start().await?;
```

### 4. 管理任务

```rust
// 列出所有任务
let jobs = scheduler.list_jobs().await?;

// 删除任务
scheduler.remove_task(job_id).await?;

// 停止调度器
scheduler.shutdown().await?;
```

---

## 📅 Cron 表达式格式

```
┌─────────── second (0-59)
│ ┌───────── minute (0-59)
│ │ ┌─────── hour (0-23)
│ │ │ ┌───── day of month (1-31)
│ │ │ │ ┌─── month (1-12)
│ │ │ │ │ ┌─ day of week (0-6, 0=Sunday)
│ │ │ │ │ │
* * * * * *
```

---

## 📖 常用 Cron 表达式

| 表达式 | 说明 |
|--------|------|
| `0 0 2 * * *` | 每天凌晨 2 点 |
| `0 0 * * * *` | 每小时整点 |
| `0 30 * * * *` | 每小时 30 分 |
| `0 */15 * * * *` | 每 15 分钟 |
| `0 0 3 * * 0` | 每周日凌晨 3 点 |
| `0 0 4 1 * *` | 每月 1 号凌晨 4 点 |
| `0 0 5 1 1 *` | 每年 1 月 1 日凌晨 5 点 |
| `0 0 0-23/2 * * *` | 每 2 小时 |

---

## 🎨 自定义任务

### 创建自定义 Cron 任务

```rust
use flux_timeseries::{ScheduledTask, TaskType};

let custom_task = ScheduledTask::new(
    "Custom Archive".to_string(),
    "0 0 */6 * * *".to_string(), // 每 6 小时
    TaskType::Archive(policy),
);

scheduler.add_task(custom_task).await?;
```

### 创建自定义任务类型

```rust
let task = ScheduledTask {
    name: "My Custom Task".to_string(),
    cron_expression: "0 30 2 * * *".to_string(), // 每天凌晨 2:30
    task_type: TaskType::Cleanup(policy),
    enabled: true,
};

scheduler.add_task(task).await?;
```

---

## 💡 最佳实践

### 1. 任务时间安排

**避免冲突**:
```rust
// ✅ 好的做法：错开任务时间
清理任务:   0 0 1 * * *  (凌晨 1 点)
归档任务:   0 0 2 * * *  (凌晨 2 点)
刷新任务:   0 0 3 * * *  (凌晨 3 点)

// ❌ 不好的做法：同时运行多个重任务
清理任务:   0 0 2 * * *
归档任务:   0 0 2 * * *  // 冲突！
```

### 2. 归档策略

**按数据量选择频率**:
```rust
// 数据量大：每日归档
let task = ScheduledTask::daily_archive(policy);

// 数据量中：每周归档
let task = ScheduledTask::weekly_archive(policy);

// 数据量小：每月归档
let task = ScheduledTask::monthly_archive(policy);
```

### 3. 清理策略

**分表清理**:
```rust
// 指标数据：保留 90 天
let metrics_task = ScheduledTask::daily_cleanup(
    CleanupPolicy::for_metrics()
);

// 日志数据：保留 30 天
let logs_task = ScheduledTask::daily_cleanup(
    CleanupPolicy::for_logs()
);

// 事件数据：保留 180 天
let events_task = ScheduledTask::daily_cleanup(
    CleanupPolicy::for_events()
);
```

### 4. 错误处理

任务失败会自动记录日志，不会影响其他任务：

```rust
// 任务失败会记录错误日志
error!(task = "Daily Archive", error = "...", "Archive task failed");

// 下次调度时会重试
```

---

## 🚀 生产环境配置

### 完整示例

```rust
use flux_timeseries::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 连接数据库
    let store = TimescaleStore::new(database_url).await?;
    let db = store.connection();

    // 创建调度器
    let scheduler = TaskScheduler::new(db.clone().into()).await?;

    // 1. 每日清理任务（凌晨 1 点）
    scheduler.add_task(ScheduledTask::daily_cleanup(
        CleanupPolicy::for_metrics()
    )).await?;

    scheduler.add_task(ScheduledTask::daily_cleanup(
        CleanupPolicy::for_logs()
    )).await?;

    // 2. 每日归档任务（凌晨 2 点）
    let archive_policy = ArchivePolicy {
        table_name: "device_metrics".to_string(),
        archive_older_than: Duration::days(365),
        destination: ArchiveDestination::S3 {
            bucket: "flux-archive".to_string(),
            region: "us-west-2".to_string(),
            prefix: "metrics".to_string(),
        },
        delete_after_archive: true,
    };
    scheduler.add_task(ScheduledTask::daily_archive(archive_policy)).await?;

    // 3. 每小时刷新降采样视图
    scheduler.add_task(ScheduledTask::hourly_downsample_refresh(
        "device_metrics_1h".to_string()
    )).await?;

    // 启动调度器
    scheduler.start().await?;

    // 保持运行
    tokio::signal::ctrl_c().await?;
    
    // 优雅关闭
    scheduler.shutdown().await?;

    Ok(())
}
```

---

## 📊 监控和日志

### 任务执行日志

```
INFO  Task scheduled: Daily Archive: device_metrics, cron=0 0 2 * * *
INFO  Executing scheduled task: Daily Archive: device_metrics
INFO  Archive task completed: 10000 rows, 50.5 MB
```

### 任务失败日志

```
ERROR Archive task failed: Daily Archive: device_metrics, error=...
```

---

## 🔍 故障排查

### 任务未执行

1. 检查调度器是否启动
2. 检查 Cron 表达式是否正确
3. 检查任务是否启用（`enabled: true`）
4. 查看日志输出

### 任务执行失败

1. 查看错误日志
2. 检查数据库连接
3. 检查磁盘空间
4. 检查权限设置

---

## ✅ 总结

**任务调度功能**:
- ✅ 基于 Cron 表达式
- ✅ 支持归档、清理、刷新任务
- ✅ 自动错误处理和日志
- ✅ 灵活的任务配置
- ✅ 生产就绪

**使用场景**:
- 定时归档历史数据
- 定时清理过期数据
- 定时刷新聚合视图
- 定时备份和维护

---

**维护者**: FLUX IOT Team  
**文档版本**: v1.0.0  
**更新日期**: 2026-02-22
