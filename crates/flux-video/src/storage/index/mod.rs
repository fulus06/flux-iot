// 索引管理（LRU 缓存）
use lru::LruCache;
use std::num::NonZeroUsize;
use super::standalone::ObjectMetadata;

/// 轻量级索引（LRU 缓存，仅缓存最近访问的对象）
pub struct LightweightIndex {
    /// LRU 缓存（最近 N 个对象）
    cache: LruCache<String, ObjectMetadata>,
}

impl LightweightIndex {
    pub fn new(cache_size: usize) -> Self {
        let capacity = NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1000).unwrap());
        
        Self {
            cache: LruCache::new(capacity),
        }
    }
    
    /// 添加对象到缓存
    pub fn put(&mut self, key: String, metadata: ObjectMetadata) {
        self.cache.put(key, metadata);
    }
    
    /// 从缓存获取对象
    pub fn get(&mut self, key: &str) -> Option<&ObjectMetadata> {
        self.cache.get(key)
    }
    
    /// 缓存大小
    pub fn len(&self) -> usize {
        self.cache.len()
    }
    
    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
    
    /// 清空缓存
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}
