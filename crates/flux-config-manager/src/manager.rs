use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::source::ConfigSource;
use crate::version::{ConfigVersion, VersionManager};

/// 配置管理器
pub struct ConfigManager<T>
where
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    /// 配置源
    source: Arc<dyn ConfigSource<T>>,
    /// 当前配置
    current: Arc<RwLock<Option<T>>>,
    /// 版本管理器
    versions: Arc<RwLock<VersionManager<T>>>,
    /// 变更通知发送器
    notifiers: Arc<RwLock<Vec<mpsc::Sender<ConfigChange<T>>>>>,
}

/// 配置变更事件
#[derive(Debug, Clone)]
pub enum ConfigChange<T> {
    Updated { old: T, new: T },
    Loaded(T),
    Deleted,
}

impl<T> ConfigManager<T>
where
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    /// 创建配置管理器
    pub fn new(source: Arc<dyn ConfigSource<T>>) -> Self {
        Self {
            source,
            current: Arc::new(RwLock::new(None)),
            versions: Arc::new(RwLock::new(VersionManager::new(10))),
            notifiers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 加载配置
    pub async fn load(&self) -> Result<T> {
        info!("Loading configuration");

        let config = self.source.load().await?;

        // 保存到当前配置
        {
            let mut current = self.current.write().await;
            *current = Some(config.clone());
        }

        // 添加到版本历史
        {
            let mut versions = self.versions.write().await;
            versions.add(config.clone(), "system".to_string(), "Initial load".to_string());
        }

        // 通知订阅者
        self.notify(ConfigChange::Loaded(config.clone())).await;

        info!("Configuration loaded successfully");
        Ok(config)
    }

    /// 重新加载配置
    pub async fn reload(&self) -> Result<()> {
        info!("Reloading configuration");

        let new_config = self.source.load().await?;
        let old_config = self.current.read().await.clone();

        if let Some(old) = old_config {
            // 保存到当前配置
            {
                let mut current = self.current.write().await;
                *current = Some(new_config.clone());
            }

            // 添加到版本历史
            {
                let mut versions = self.versions.write().await;
                versions.add(
                    new_config.clone(),
                    "system".to_string(),
                    "Hot reload".to_string(),
                );
            }

            // 通知订阅者
            self.notify(ConfigChange::Updated {
                old,
                new: new_config,
            })
            .await;

            info!("Configuration reloaded successfully");
        } else {
            warn!("No current configuration to reload");
        }

        Ok(())
    }

    /// 更新配置
    pub async fn update(&self, config: T, author: String, comment: String) -> Result<()> {
        info!("Updating configuration");

        // 保存到源
        self.source.save(&config).await?;

        let old_config = self.current.read().await.clone();

        // 更新当前配置
        {
            let mut current = self.current.write().await;
            *current = Some(config.clone());
        }

        // 添加到版本历史
        {
            let mut versions = self.versions.write().await;
            versions.add(config.clone(), author, comment);
        }

        // 通知订阅者
        if let Some(old) = old_config {
            self.notify(ConfigChange::Updated {
                old,
                new: config,
            })
            .await;
        } else {
            self.notify(ConfigChange::Loaded(config)).await;
        }

        info!("Configuration updated successfully");
        Ok(())
    }

    /// 回滚到指定版本
    pub async fn rollback(&self, version: u64) -> Result<()> {
        info!("Rolling back to version {}", version);

        let config = {
            let versions = self.versions.read().await;
            versions
                .get(version)
                .ok_or_else(|| anyhow::anyhow!("Version {} not found", version))?
                .config
                .clone()
        };

        // 保存到源
        self.source.save(&config).await?;

        let old_config = self.current.read().await.clone();

        // 更新当前配置
        {
            let mut current = self.current.write().await;
            *current = Some(config.clone());
        }

        // 通知订阅者
        if let Some(old) = old_config {
            self.notify(ConfigChange::Updated {
                old,
                new: config,
            })
            .await;
        }

        info!("Rolled back to version {} successfully", version);
        Ok(())
    }

    /// 获取当前配置
    pub async fn get(&self) -> Option<T> {
        self.current.read().await.clone()
    }

    /// 列出版本历史
    pub async fn list_versions(&self) -> Vec<ConfigVersion<T>> {
        let versions = self.versions.read().await;
        versions.list().to_vec()
    }

    /// 订阅配置变更
    pub async fn subscribe(&self) -> mpsc::Receiver<ConfigChange<T>> {
        let (tx, rx) = mpsc::channel(10);
        let mut notifiers = self.notifiers.write().await;
        notifiers.push(tx);
        rx
    }

    /// 通知所有订阅者
    async fn notify(&self, change: ConfigChange<T>) {
        let notifiers = self.notifiers.read().await;
        for tx in notifiers.iter() {
            if let Err(e) = tx.send(change.clone()).await {
                warn!("Failed to notify subscriber: {}", e);
            }
        }
    }

    /// 启动配置监听
    pub async fn start_watching(self: Arc<Self>) -> Result<()> {
        info!("Starting configuration watcher");

        let mut watcher = self.source.watch().await?;

        tokio::spawn(async move {
            loop {
                match watcher.recv().await {
                    Some(()) => {
                        debug!("Configuration change detected");
                        if let Err(e) = self.reload().await {
                            warn!("Failed to reload configuration: {}", e);
                        }
                    }
                    None => {
                        warn!("Configuration watcher closed");
                        break;
                    }
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::MockConfigSource;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        value: String,
    }

    #[tokio::test]
    async fn test_config_manager_load() {
        let source = Arc::new(MockConfigSource::new(TestConfig {
            value: "test".to_string(),
        }));

        let manager = ConfigManager::new(source);
        let config = manager.load().await.unwrap();

        assert_eq!(config.value, "test");
    }

    #[tokio::test]
    async fn test_config_manager_update() {
        let source = Arc::new(MockConfigSource::new(TestConfig {
            value: "initial".to_string(),
        }));

        let manager = ConfigManager::new(source);
        manager.load().await.unwrap();

        let new_config = TestConfig {
            value: "updated".to_string(),
        };

        manager
            .update(new_config.clone(), "test".to_string(), "Update test".to_string())
            .await
            .unwrap();

        let current = manager.get().await.unwrap();
        assert_eq!(current.value, "updated");
    }

    #[tokio::test]
    async fn test_config_manager_rollback() {
        let source = Arc::new(MockConfigSource::new(TestConfig {
            value: "v1".to_string(),
        }));

        let manager = ConfigManager::new(source);
        manager.load().await.unwrap();

        // 更新到 v2
        manager
            .update(
                TestConfig {
                    value: "v2".to_string(),
                },
                "test".to_string(),
                "v2".to_string(),
            )
            .await
            .unwrap();

        // 回滚到 v1
        manager.rollback(1).await.unwrap();

        let current = manager.get().await.unwrap();
        assert_eq!(current.value, "v1");
    }
}
