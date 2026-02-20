use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 配置版本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigVersion<T> {
    pub version: u64,
    pub config: T,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub comment: String,
}

/// 版本管理器
pub struct VersionManager<T> {
    versions: Vec<ConfigVersion<T>>,
    max_versions: usize,
    next_version: u64,
}

impl<T: Clone> VersionManager<T> {
    pub fn new(max_versions: usize) -> Self {
        Self {
            versions: Vec::new(),
            max_versions,
            next_version: 1,
        }
    }

    /// 添加新版本
    pub fn add(&mut self, config: T, author: String, comment: String) {
        let version = ConfigVersion {
            version: self.next_version,
            config,
            timestamp: Utc::now(),
            author,
            comment,
        };

        self.versions.push(version);
        self.next_version += 1;

        // 保持最大版本数限制
        if self.versions.len() > self.max_versions {
            self.versions.remove(0);
        }
    }

    /// 获取指定版本
    pub fn get(&self, version: u64) -> Option<&ConfigVersion<T>> {
        self.versions.iter().find(|v| v.version == version)
    }

    /// 列出所有版本
    pub fn list(&self) -> &[ConfigVersion<T>] {
        &self.versions
    }

    /// 获取最新版本
    pub fn latest(&self) -> Option<&ConfigVersion<T>> {
        self.versions.last()
    }

    /// 清空版本历史
    pub fn clear(&mut self) {
        self.versions.clear();
        self.next_version = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_manager() {
        let mut manager = VersionManager::new(3);

        manager.add("v1".to_string(), "user1".to_string(), "First".to_string());
        manager.add("v2".to_string(), "user2".to_string(), "Second".to_string());
        manager.add("v3".to_string(), "user3".to_string(), "Third".to_string());

        assert_eq!(manager.list().len(), 3);
        assert_eq!(manager.get(1).unwrap().config, "v1");
        assert_eq!(manager.latest().unwrap().config, "v3");

        // 超过最大版本数
        manager.add("v4".to_string(), "user4".to_string(), "Fourth".to_string());
        assert_eq!(manager.list().len(), 3);
        assert!(manager.get(1).is_none()); // v1 被移除
        assert_eq!(manager.get(2).unwrap().config, "v2");
    }
}
