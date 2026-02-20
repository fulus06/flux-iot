use anyhow::Result;
use async_trait::async_trait;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::source::{ConfigSource, ConfigWatcher};

/// 文件配置源
pub struct FileSource<T> {
    path: PathBuf,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> FileSource<T> {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<T> ConfigSource<T> for FileSource<T>
where
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    async fn load(&self) -> Result<T> {
        debug!("Loading config from file: {:?}", self.path);

        let content = fs::read_to_string(&self.path).await?;

        let config = if self.path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content)?
        } else {
            // 默认使用 TOML
            toml::from_str(&content)?
        };

        Ok(config)
    }

    async fn save(&self, config: &T) -> Result<()> {
        debug!("Saving config to file: {:?}", self.path);

        let content = if self.path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::to_string_pretty(config)?
        } else {
            // 默认使用 TOML
            toml::to_string_pretty(config)?
        };

        fs::write(&self.path, content).await?;

        Ok(())
    }

    async fn watch(&self) -> Result<ConfigWatcher> {
        let (tx, rx) = mpsc::channel(10);
        let path = self.path.clone();

        std::thread::spawn(move || {
            let (notify_tx, notify_rx) = std::sync::mpsc::channel();

            let mut watcher: RecommendedWatcher =
                Watcher::new(notify_tx, notify::Config::default()).unwrap();

            watcher
                .watch(&path, RecursiveMode::NonRecursive)
                .unwrap();

            debug!("File watcher started for: {:?}", path);

            loop {
                match notify_rx.recv() {
                    Ok(Ok(Event { kind, .. })) => {
                        use notify::EventKind::*;
                        match kind {
                            Modify(_) | Create(_) => {
                                debug!("Config file changed: {:?}", path);
                                if tx.blocking_send(()).is_err() {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Watch error: {}", e);
                    }
                    Err(e) => {
                        error!("Channel error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(ConfigWatcher::new(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::NamedTempFile;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        name: String,
        value: i32,
    }

    #[tokio::test]
    async fn test_file_source_toml() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().with_extension("toml");

        let source = FileSource::new(&path);

        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };

        source.save(&config).await.unwrap();
        let loaded: TestConfig = source.load().await.unwrap();

        assert_eq!(loaded, config);

        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn test_file_source_json() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().with_extension("json");

        let source = FileSource::new(&path);

        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };

        source.save(&config).await.unwrap();
        let loaded: TestConfig = source.load().await.unwrap();

        assert_eq!(loaded, config);

        std::fs::remove_file(&path).ok();
    }
}
