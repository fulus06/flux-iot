use anyhow::Result;
use flux_plugin::PluginManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::sync::mpsc::channel;

/// 插件加载器服务
/// 
/// 负责从指定目录加载 Wasm 插件，提供插件生命周期管理
#[derive(Clone)]
pub struct PluginLoader {
    /// 插件目录
    plugin_dir: PathBuf,
    
    /// 插件管理器
    plugin_manager: Arc<PluginManager>,
}

impl PluginLoader {
    /// 创建新的插件加载器
    pub fn new(plugin_dir: impl Into<PathBuf>, plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            plugin_dir: plugin_dir.into(),
            plugin_manager,
        }
    }

    /// 加载所有插件
    /// 
    /// 扫描插件目录，加载所有 .wasm 文件
    pub async fn load_all(&self) -> Result<LoadResult> {
        let plugin_dir = &self.plugin_dir;
        
        info!(directory = %plugin_dir.display(), "Loading plugins");

        let mut result = LoadResult {
            total: 0,
            loaded: 0,
            failed: Vec::new(),
        };

        // 检查目录是否存在
        if !plugin_dir.exists() {
            warn!(directory = %plugin_dir.display(), "Plugin directory does not exist");
            return Ok(result);
        }

        // 读取目录
        let entries = match std::fs::read_dir(plugin_dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!(directory = %plugin_dir.display(), error = %e, "Failed to read plugin directory");
                return Err(e.into());
            }
        };

        // 遍历所有文件
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            
            // 只处理 .wasm 文件
            if !path.extension().is_some_and(|ext| ext == "wasm") {
                continue;
            }

            result.total += 1;

            // 加载插件
            match self.load_plugin(&path).await {
                Ok(plugin_name) => {
                    info!(plugin = %plugin_name, path = %path.display(), "Plugin loaded successfully");
                    result.loaded += 1;
                }
                Err(e) => {
                    error!(path = %path.display(), error = %e, "Failed to load plugin");
                    result.failed.push(PluginLoadError {
                        path: path.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        info!(
            total = result.total,
            loaded = result.loaded,
            failed = result.failed.len(),
            "Plugin loading completed"
        );

        Ok(result)
    }

    /// 加载单个插件
    async fn load_plugin(&self, path: &Path) -> Result<String> {
        debug!(path = %path.display(), "Loading plugin");

        // 读取插件文件
        let bytes = tokio::fs::read(path).await?;

        // 获取插件名称（文件名，不含扩展名）
        let plugin_name = path
            .file_stem()
            .ok_or_else(|| anyhow::anyhow!("Invalid plugin filename"))?
            .to_string_lossy()
            .to_string();

        // 加载到插件管理器
        self.plugin_manager.load_plugin(&plugin_name, &bytes)?;

        Ok(plugin_name)
    }

    /// 重新加载所有插件
    /// 
    /// 用于热更新场景
    pub async fn reload_all(&self) -> Result<LoadResult> {
        info!("Reloading all plugins");
        
        // 注意：当前 PluginManager 不支持卸载，这里只是重新加载
        // 实际生产环境可能需要先卸载旧插件
        self.load_all().await
    }

    /// 加载指定的插件文件
    pub async fn load_file(&self, path: impl AsRef<Path>) -> Result<String> {
        let path = path.as_ref();
        self.load_plugin(path).await
    }

    /// 获取插件目录
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// 启动插件热更新监控
    /// 
    /// 监听插件目录中的 .wasm 文件变化，自动重新加载插件
    pub async fn start_hot_reload(&self) -> anyhow::Result<()> {
        let plugin_dir = self.plugin_dir.clone();
        let loader = Arc::new(self.clone());
        
        info!(
            plugin_dir = %plugin_dir.display(),
            "Starting plugin hot reload monitoring"
        );
        
        // 创建文件监控通道
        let (tx, rx) = channel();
        
        // 创建文件监控器
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })?;
        
        // 监控插件目录
        watcher.watch(&plugin_dir, RecursiveMode::NonRecursive)?;
        
        info!("Plugin file watcher started");
        
        // 启动后台任务处理文件变化事件
        tokio::task::spawn_blocking(move || {
            // 保持 watcher 存活
            let _watcher = watcher;
            
            loop {
                match rx.recv() {
                    Ok(event) => {
                        // 检查是否是 .wasm 文件的修改或创建事件
                        let should_reload = match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) => {
                                event.paths.iter().any(|p| {
                                    p.extension()
                                        .and_then(|e| e.to_str())
                                        .map(|e| e == "wasm")
                                        .unwrap_or(false)
                                })
                            }
                            _ => false,
                        };
                        
                        if should_reload {
                            info!(
                                paths = ?event.paths,
                                "Plugin file changed, reloading..."
                            );
                            
                            // 在异步上下文中重新加载插件
                            let loader_clone = loader.clone();
                            tokio::spawn(async move {
                                match loader_clone.reload_all().await {
                                    Ok(result) => {
                                        info!(
                                            total = result.total,
                                            loaded = result.loaded,
                                            failed = result.failed.len(),
                                            "Plugins reloaded successfully"
                                        );
                                    }
                                    Err(e) => {
                                        error!(
                                            error = %e,
                                            "Failed to reload plugins"
                                        );
                                    }
                                }
                            });
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "File watcher channel error");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
}

/// 插件加载结果
#[derive(Debug, Clone)]
pub struct LoadResult {
    /// 发现的插件总数
    pub total: usize,
    
    /// 成功加载的插件数
    pub loaded: usize,
    
    /// 加载失败的插件
    pub failed: Vec<PluginLoadError>,
}

impl LoadResult {
    /// 是否所有插件都加载成功
    pub fn is_all_success(&self) -> bool {
        self.failed.is_empty()
    }

    /// 成功率
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            1.0
        } else {
            self.loaded as f64 / self.total as f64
        }
    }
}

/// 插件加载错误
#[derive(Debug, Clone)]
pub struct PluginLoadError {
    /// 插件路径
    pub path: PathBuf,
    
    /// 错误信息
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_plugin_loader_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_manager = Arc::new(PluginManager::new().unwrap());
        let loader = PluginLoader::new(temp_dir.path(), plugin_manager);

        let result = loader.load_all().await.unwrap();
        assert_eq!(result.total, 0);
        assert_eq!(result.loaded, 0);
        assert!(result.is_all_success());
    }

    #[tokio::test]
    async fn test_plugin_loader_nonexistent_directory() {
        let plugin_manager = Arc::new(PluginManager::new().unwrap());
        let loader = PluginLoader::new("/nonexistent/path", plugin_manager);

        let result = loader.load_all().await.unwrap();
        assert_eq!(result.total, 0);
    }

    #[tokio::test]
    async fn test_load_result_success_rate() {
        let result = LoadResult {
            total: 10,
            loaded: 8,
            failed: vec![
                PluginLoadError {
                    path: PathBuf::from("plugin1.wasm"),
                    error: "error1".to_string(),
                },
                PluginLoadError {
                    path: PathBuf::from("plugin2.wasm"),
                    error: "error2".to_string(),
                },
            ],
        };

        assert_eq!(result.success_rate(), 0.8);
        assert!(!result.is_all_success());
    }
}
