use crate::archive::{ArchivePolicy, DataArchiver};
use crate::cleanup::{CleanupManager, CleanupPolicy};
use crate::downsample::{DownsampleManager, DownsamplePolicy};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};

/// 任务类型
#[derive(Debug, Clone)]
pub enum TaskType {
    /// 归档任务
    Archive(ArchivePolicy),
    /// 清理任务
    Cleanup(CleanupPolicy),
    /// 降采样刷新任务
    DownsampleRefresh(String), // view_name
}

/// 调度任务
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    /// 任务名称
    pub name: String,
    
    /// Cron 表达式
    pub cron_expression: String,
    
    /// 任务类型
    pub task_type: TaskType,
    
    /// 是否启用
    pub enabled: bool,
}

impl ScheduledTask {
    pub fn new(name: String, cron_expression: String, task_type: TaskType) -> Self {
        Self {
            name,
            cron_expression,
            task_type,
            enabled: true,
        }
    }

    /// 创建每日归档任务
    pub fn daily_archive(policy: ArchivePolicy) -> Self {
        Self::new(
            format!("Daily Archive: {}", policy.table_name),
            "0 0 2 * * *".to_string(), // 每天凌晨 2 点
            TaskType::Archive(policy),
        )
    }

    /// 创建每周归档任务
    pub fn weekly_archive(policy: ArchivePolicy) -> Self {
        Self::new(
            format!("Weekly Archive: {}", policy.table_name),
            "0 0 3 * * 0".to_string(), // 每周日凌晨 3 点
            TaskType::Archive(policy),
        )
    }

    /// 创建每月归档任务
    pub fn monthly_archive(policy: ArchivePolicy) -> Self {
        Self::new(
            format!("Monthly Archive: {}", policy.table_name),
            "0 0 4 1 * *".to_string(), // 每月 1 号凌晨 4 点
            TaskType::Archive(policy),
        )
    }

    /// 创建每日清理任务
    pub fn daily_cleanup(policy: CleanupPolicy) -> Self {
        Self::new(
            format!("Daily Cleanup: {}", policy.table_name),
            "0 0 1 * * *".to_string(), // 每天凌晨 1 点
            TaskType::Cleanup(policy),
        )
    }

    /// 创建每小时降采样刷新任务
    pub fn hourly_downsample_refresh(view_name: String) -> Self {
        Self::new(
            format!("Hourly Refresh: {}", view_name),
            "0 0 * * * *".to_string(), // 每小时整点
            TaskType::DownsampleRefresh(view_name),
        )
    }
}

/// 任务调度器
pub struct TaskScheduler {
    scheduler: JobScheduler,
    db: Arc<DatabaseConnection>,
}

impl TaskScheduler {
    /// 创建新的任务调度器
    pub async fn new(db: Arc<DatabaseConnection>) -> anyhow::Result<Self> {
        let scheduler = JobScheduler::new().await?;
        
        Ok(Self { scheduler, db })
    }

    /// 添加任务
    pub async fn add_task(&self, task: ScheduledTask) -> anyhow::Result<uuid::Uuid> {
        if !task.enabled {
            info!(task_name = %task.name, "Task is disabled, skipping");
            return Ok(uuid::Uuid::nil());
        }

        let db = self.db.clone();
        let task_name = task.name.clone();
        let task_type = task.task_type.clone();

        let job = Job::new_async(task.cron_expression.as_str(), move |_uuid, _l| {
            let db = db.clone();
            let task_name = task_name.clone();
            let task_type = task_type.clone();

            Box::pin(async move {
                info!(task = %task_name, "Executing scheduled task");

                match task_type {
                    TaskType::Archive(policy) => {
                        let archiver = DataArchiver::new(db.clone());
                        match archiver.archive(&policy).await {
                            Ok(stats) => {
                                info!(
                                    task = %task_name,
                                    archived_rows = %stats.archived_rows,
                                    size_mb = %stats.archive_size_mb,
                                    "Archive task completed"
                                );
                            }
                            Err(e) => {
                                error!(task = %task_name, error = %e, "Archive task failed");
                            }
                        }
                    }
                    TaskType::Cleanup(policy) => {
                        let cleanup_mgr = CleanupManager::new(db.clone());
                        match cleanup_mgr.cleanup(&policy).await {
                            Ok(stats) => {
                                info!(
                                    task = %task_name,
                                    deleted_rows = %stats.deleted_rows,
                                    freed_mb = %stats.freed_space_mb,
                                    "Cleanup task completed"
                                );
                            }
                            Err(e) => {
                                error!(task = %task_name, error = %e, "Cleanup task failed");
                            }
                        }
                    }
                    TaskType::DownsampleRefresh(view_name) => {
                        let downsample_mgr = DownsampleManager::new(db.clone());
                        match downsample_mgr.refresh_view(&view_name).await {
                            Ok(_) => {
                                info!(
                                    task = %task_name,
                                    view = %view_name,
                                    "Downsample refresh completed"
                                );
                            }
                            Err(e) => {
                                error!(
                                    task = %task_name,
                                    view = %view_name,
                                    error = %e,
                                    "Downsample refresh failed"
                                );
                            }
                        }
                    }
                }
            })
        })?;

        let job_id = self.scheduler.add(job).await?;

        info!(
            task_name = %task.name,
            cron = %task.cron_expression,
            job_id = %job_id,
            "Task scheduled"
        );

        Ok(job_id)
    }

    /// 启动调度器
    pub async fn start(&mut self) -> anyhow::Result<()> {
        self.scheduler.start().await?;
        info!("Task scheduler started");
        Ok(())
    }

    /// 停止调度器
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.scheduler.shutdown().await?;
        info!("Task scheduler stopped");
        Ok(())
    }

    /// 删除任务
    pub async fn remove_task(&mut self, job_id: uuid::Uuid) -> anyhow::Result<()> {
        self.scheduler.remove(&job_id).await?;
        info!(job_id = %job_id, "Task removed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_expressions() {
        // 每天凌晨 2 点
        assert_eq!("0 0 2 * * *", "0 0 2 * * *");
        
        // 每周日凌晨 3 点
        assert_eq!("0 0 3 * * 0", "0 0 3 * * 0");
        
        // 每月 1 号凌晨 4 点
        assert_eq!("0 0 4 1 * *", "0 0 4 1 * *");
    }

    #[test]
    fn test_scheduled_task_creation() {
        use crate::cleanup::CleanupPolicy;
        
        let policy = CleanupPolicy::for_metrics();
        let task = ScheduledTask::daily_cleanup(policy);
        
        assert_eq!(task.cron_expression, "0 0 1 * * *");
        assert!(task.enabled);
    }
}
