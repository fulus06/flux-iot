use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// 配置源抽象
#[async_trait]
pub trait ConfigSource<T>: Send + Sync
where
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    /// 加载配置
    async fn load(&self) -> Result<T>;

    /// 保存配置
    async fn save(&self, config: &T) -> Result<()>;

    /// 监听配置变更
    async fn watch(&self) -> Result<ConfigWatcher>;
}

/// 配置监听器
pub struct ConfigWatcher {
    rx: mpsc::Receiver<()>,
}

impl ConfigWatcher {
    pub fn new(rx: mpsc::Receiver<()>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<()> {
        self.rx.recv().await
    }
}

/// Mock 配置源（用于测试）
#[cfg(test)]
pub struct MockConfigSource<T> {
    config: std::sync::Arc<tokio::sync::RwLock<T>>,
}

#[cfg(test)]
impl<T> MockConfigSource<T>
where
    T: Clone + Send + Sync,
{
    pub fn new(config: T) -> Self {
        Self {
            config: std::sync::Arc::new(tokio::sync::RwLock::new(config)),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl<T> ConfigSource<T> for MockConfigSource<T>
where
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    async fn load(&self) -> Result<T> {
        Ok(self.config.read().await.clone())
    }

    async fn save(&self, config: &T) -> Result<()> {
        let mut current = self.config.write().await;
        *current = config.clone();
        Ok(())
    }

    async fn watch(&self) -> Result<ConfigWatcher> {
        let (_tx, rx) = mpsc::channel(1);
        Ok(ConfigWatcher::new(rx))
    }
}
