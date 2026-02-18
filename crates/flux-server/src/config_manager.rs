use crate::config::AppConfig;
use crate::config_provider::AppConfigProvider;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

pub struct ConfigManager {
    provider: Arc<dyn AppConfigProvider>,
    tx: watch::Sender<AppConfig>,
    version: RwLock<i64>,
}

impl ConfigManager {
    pub fn new(provider: Arc<dyn AppConfigProvider>, initial: AppConfig, version: i64) -> Self {
        let (tx, _rx) = watch::channel(initial);
        Self {
            provider,
            tx,
            version: RwLock::new(version),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<AppConfig> {
        self.tx.subscribe()
    }

    pub fn current(&self) -> AppConfig {
        self.tx.borrow().clone()
    }

    pub fn start_polling(self: Arc<Self>, interval: std::time::Duration) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;

                let new_ver = match self.provider.version().await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("Config version check failed: {}", e);
                        continue;
                    }
                };

                let mut ver_guard = self.version.write().await;
                if new_ver == *ver_guard {
                    continue;
                }

                match self.provider.load().await {
                    Ok(cfg) => {
                        *ver_guard = new_ver;
                        let _ = self.tx.send(cfg);
                        tracing::info!("Config reloaded (version={})", new_ver);
                    }
                    Err(e) => {
                        tracing::warn!("Config reload failed (version={}): {}", new_ver, e);
                    }
                }
            }
        });
    }
}
