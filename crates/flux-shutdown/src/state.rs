use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum StateError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("State not found")]
    NotFound,
}

/// 状态管理器
pub struct StateManager<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    state: Arc<RwLock<T>>,
    checkpoint_path: PathBuf,
}

impl<T> StateManager<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    pub fn new(state: T, checkpoint_path: impl AsRef<Path>) -> Self {
        Self {
            state: Arc::new(RwLock::new(state)),
            checkpoint_path: checkpoint_path.as_ref().to_path_buf(),
        }
    }

    /// 获取状态的只读引用
    pub async fn get(&self) -> tokio::sync::RwLockReadGuard<'_, T> {
        self.state.read().await
    }

    /// 获取状态的可写引用
    pub async fn get_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, T> {
        self.state.write().await
    }

    /// 保存检查点
    pub async fn save_checkpoint(&self) -> Result<(), StateError> {
        let state = self.state.read().await;
        let json = serde_json::to_string_pretty(&*state)?;

        // 创建父目录
        if let Some(parent) = self.checkpoint_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 写入临时文件
        let temp_path = self.checkpoint_path.with_extension("tmp");
        fs::write(&temp_path, json).await?;

        // 原子性重命名
        fs::rename(&temp_path, &self.checkpoint_path).await?;

        info!("State checkpoint saved to {:?}", self.checkpoint_path);
        Ok(())
    }

    /// 加载检查点
    pub async fn load_checkpoint(&self) -> Result<(), StateError> {
        if !self.checkpoint_path.exists() {
            return Err(StateError::NotFound);
        }

        let json = fs::read_to_string(&self.checkpoint_path).await?;
        let loaded_state: T = serde_json::from_str(&json)?;

        let mut state = self.state.write().await;
        *state = loaded_state;

        info!("State checkpoint loaded from {:?}", self.checkpoint_path);
        Ok(())
    }

    /// 删除检查点
    pub async fn delete_checkpoint(&self) -> Result<(), StateError> {
        if self.checkpoint_path.exists() {
            fs::remove_file(&self.checkpoint_path).await?;
            info!("State checkpoint deleted: {:?}", self.checkpoint_path);
        }
        Ok(())
    }

    /// 检查点是否存在
    pub fn checkpoint_exists(&self) -> bool {
        self.checkpoint_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        counter: u64,
        message: String,
    }

    #[tokio::test]
    async fn test_state_manager() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let initial_state = TestState {
            counter: 42,
            message: "Hello".to_string(),
        };

        let manager = StateManager::new(initial_state.clone(), &path);

        // 保存检查点
        manager.save_checkpoint().await.unwrap();

        // 修改状态
        {
            let mut state = manager.get_mut().await;
            state.counter = 100;
            state.message = "Modified".to_string();
        }

        // 加载检查点（恢复原始状态）
        manager.load_checkpoint().await.unwrap();

        let state = manager.get().await;
        assert_eq!(state.counter, 42);
        assert_eq!(state.message, "Hello");
    }

    #[tokio::test]
    async fn test_checkpoint_not_found() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().with_extension("nonexistent");

        let state = TestState {
            counter: 0,
            message: String::new(),
        };

        let manager = StateManager::new(state, &path);

        // 加载不存在的检查点应该失败
        assert!(manager.load_checkpoint().await.is_err());
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let state = TestState {
            counter: 42,
            message: "Test".to_string(),
        };

        let manager = StateManager::new(state, &path);

        // 保存检查点
        manager.save_checkpoint().await.unwrap();
        assert!(manager.checkpoint_exists());

        // 删除检查点
        manager.delete_checkpoint().await.unwrap();
        assert!(!manager.checkpoint_exists());
    }
}
