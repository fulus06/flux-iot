# TS 分片存储架构设计方案

## 1. 架构概述

### 1.1 层次划分

```
应用层 (flux-rtmpd)
    ↓
业务抽象层 (SegmentStorage trait)
    ↓
基础设施层 (flux-storage)
    ├─ StorageManager (存储管理器)
    ├─ StorageBackend trait (存储后端抽象)
    │   ├─ LocalBackend (本地文件系统)
    │   ├─ S3Backend (AWS S3)
    │   ├─ OSSBackend (阿里云 OSS)
    │   └─ RedisBackend (Redis 缓存)
    └─ StoragePool (存储池)
```

### 1.2 职责划分

| 层次 | 组件 | 职责 |
|------|------|------|
| **基础设施层** | `StorageBackend` | 提供统一的存储后端接口（读/写/删除/列表） |
| | `StorageManager` | 管理多个存储池，负载均衡，健康检查 |
| | `StoragePool` | 封装单个存储后端，提供配置和状态管理 |
| **业务抽象层** | `SegmentStorage` | 定义分片存储的业务逻辑（路径构造、索引管理） |
| **应用层** | `HlsManager` | 使用 SegmentStorage 保存/加载分片 |

---

## 2. 核心接口设计

### 2.1 StorageBackend Trait（基础设施层）

```rust
// flux-storage/src/backend/mod.rs

#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 写入文件
    async fn write(&self, path: &str, data: &[u8]) -> Result<()>;
    
    /// 读取文件
    async fn read(&self, path: &str) -> Result<Bytes>;
    
    /// 删除文件
    async fn delete(&self, path: &str) -> Result<()>;
    
    /// 列出文件
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
    
    /// 检查文件是否存在
    async fn exists(&self, path: &str) -> Result<bool>;
    
    /// 获取后端类型
    fn backend_type(&self) -> &str;
}
```

### 2.2 StorageBackend 实现

#### 2.2.1 LocalBackend（本地文件系统）

```rust
// flux-storage/src/backend/local.rs

pub struct LocalBackend {
    base_dir: PathBuf,
}

impl LocalBackend {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
    
    fn resolve_path(&self, path: &str) -> PathBuf {
        self.base_dir.join(path)
    }
}

#[async_trait]
impl StorageBackend for LocalBackend {
    async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let full_path = self.resolve_path(path);
        
        // 创建父目录
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // 写入文件
        tokio::fs::write(&full_path, data).await?;
        Ok(())
    }
    
    async fn read(&self, path: &str) -> Result<Bytes> {
        let full_path = self.resolve_path(path);
        let data = tokio::fs::read(&full_path).await?;
        Ok(Bytes::from(data))
    }
    
    async fn delete(&self, path: &str) -> Result<()> {
        let full_path = self.resolve_path(path);
        tokio::fs::remove_file(&full_path).await?;
        Ok(())
    }
    
    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let dir = self.resolve_path(prefix);
        let mut entries = Vec::new();
        
        let mut read_dir = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                entries.push(name.to_string());
            }
        }
        
        Ok(entries)
    }
    
    async fn exists(&self, path: &str) -> Result<bool> {
        let full_path = self.resolve_path(path);
        Ok(full_path.exists())
    }
    
    fn backend_type(&self) -> &str {
        "local"
    }
}
```

#### 2.2.2 S3Backend（AWS S3）

```rust
// flux-storage/src/backend/s3.rs

pub struct S3Backend {
    bucket: String,
    region: String,
    client: S3Client,
}

impl S3Backend {
    pub async fn new(bucket: String, region: String) -> Result<Self> {
        // 初始化 S3 客户端
        let config = aws_config::from_env()
            .region(Region::new(region.clone()))
            .load()
            .await;
        
        let client = S3Client::new(&config);
        
        Ok(Self {
            bucket,
            region,
            client,
        })
    }
}

#[async_trait]
impl StorageBackend for S3Backend {
    async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .body(ByteStream::from(Bytes::copy_from_slice(data)))
            .send()
            .await?;
        
        Ok(())
    }
    
    async fn read(&self, path: &str) -> Result<Bytes> {
        let output = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await?;
        
        let data = output.body.collect().await?.into_bytes();
        Ok(data)
    }
    
    async fn delete(&self, path: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await?;
        
        Ok(())
    }
    
    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let output = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .send()
            .await?;
        
        let keys = output
            .contents()
            .iter()
            .filter_map(|obj| obj.key().map(|k| k.to_string()))
            .collect();
        
        Ok(keys)
    }
    
    async fn exists(&self, path: &str) -> Result<bool> {
        match self.client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    
    fn backend_type(&self) -> &str {
        "s3"
    }
}
```

#### 2.2.3 OSSBackend（阿里云 OSS）

```rust
// flux-storage/src/backend/oss.rs

pub struct OSSBackend {
    bucket: String,
    endpoint: String,
    client: OSSClient,
}

// 实现类似 S3Backend
```

### 2.3 StoragePool 扩展

```rust
// flux-storage/src/pool.rs

pub struct StoragePool {
    pub id: String,
    pub config: PoolConfig,
    pub backend: Arc<dyn StorageBackend>,  // 新增：使用 Backend
    pub status: Arc<RwLock<HealthStatus>>,
    pub metrics: Arc<RwLock<PoolMetrics>>,
}

impl StoragePool {
    pub fn new(config: PoolConfig, backend: Arc<dyn StorageBackend>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            backend,
            status: Arc::new(RwLock::new(HealthStatus::Healthy)),
            metrics: Arc::new(RwLock::new(PoolMetrics::default())),
        }
    }
    
    /// 写入文件
    pub async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        self.backend.write(path, data).await
    }
    
    /// 读取文件
    pub async fn read(&self, path: &str) -> Result<Bytes> {
        self.backend.read(path).await
    }
}
```

### 2.4 StorageManager 扩展

```rust
// flux-storage/src/manager.rs

impl StorageManager {
    /// 初始化存储池（支持多种后端）
    pub async fn initialize_with_backends(
        &self,
        pool_configs: Vec<(PoolConfig, Arc<dyn StorageBackend>)>,
    ) -> Result<()> {
        let mut pools = self.pools.write().await;
        
        for (config, backend) in pool_configs {
            let pool = StoragePool::new(config.clone(), backend);
            pools.insert(config.name.clone(), pool);
        }
        
        Ok(())
    }
    
    /// 写入文件到指定池
    pub async fn write_to_pool(
        &self,
        pool_name: &str,
        path: &str,
        data: &[u8],
    ) -> Result<()> {
        let pools = self.pools.read().await;
        let pool = pools.get(pool_name)
            .ok_or_else(|| anyhow!("Pool not found: {}", pool_name))?;
        
        pool.write(path, data).await
    }
    
    /// 从指定池读取文件
    pub async fn read_from_pool(
        &self,
        pool_name: &str,
        path: &str,
    ) -> Result<Bytes> {
        let pools = self.pools.read().await;
        let pool = pools.get(pool_name)
            .ok_or_else(|| anyhow!("Pool not found: {}", pool_name))?;
        
        pool.read(path).await
    }
    
    /// 选择最佳池并写入
    pub async fn write_with_selection(
        &self,
        path: &str,
        data: &[u8],
    ) -> Result<String> {
        // 选择最佳存储池
        let pool_name = self.select_best_pool(data.len() as u64).await?;
        
        // 写入文件
        self.write_to_pool(&pool_name, path, data).await?;
        
        Ok(pool_name)
    }
    
    fn select_best_pool(&self, required_space: u64) -> Result<String> {
        // 负载均衡逻辑（已有）
        // ...
    }
}
```

---

## 3. SegmentStorage 重构

### 3.1 新的 SegmentStorage 实现

```rust
// flux-storage/src/segment.rs

pub struct SegmentStorageImpl {
    storage_manager: Arc<StorageManager>,
}

impl SegmentStorageImpl {
    pub fn new(storage_manager: Arc<StorageManager>) -> Self {
        Self { storage_manager }
    }
    
    /// 构造分片路径（业务逻辑）
    fn build_segment_path(&self, stream_id: &str, segment_id: u64) -> String {
        format!("hls/{}/segment_{}.ts", stream_id, segment_id)
    }
}

#[async_trait]
impl SegmentStorage for SegmentStorageImpl {
    async fn save_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
        data: &[u8],
    ) -> Result<String> {
        // 1. 构造路径（业务逻辑）
        let path = self.build_segment_path(stream_id, segment_id);
        
        // 2. 选择存储池并写入（使用 StorageManager）
        let pool_name = self.storage_manager
            .write_with_selection(&path, data)
            .await?;
        
        info!(
            stream_id = %stream_id,
            segment_id = segment_id,
            pool = %pool_name,
            size = data.len(),
            "Segment saved"
        );
        
        Ok(format!("segment_{}.ts", segment_id))
    }
    
    async fn load_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<Bytes> {
        let path = self.build_segment_path(stream_id, segment_id);
        
        // 尝试从所有池中加载（容错）
        // TODO: 可以添加索引记录每个分片在哪个池
        self.storage_manager.read_from_any_pool(&path).await
    }
    
    async fn delete_segment(
        &self,
        stream_id: &str,
        segment_id: u64,
    ) -> Result<()> {
        let path = self.build_segment_path(stream_id, segment_id);
        self.storage_manager.delete_from_any_pool(&path).await
    }
    
    async fn list_segments(&self, stream_id: &str) -> Result<Vec<u64>> {
        let prefix = format!("hls/{}/", stream_id);
        let files = self.storage_manager.list_from_any_pool(&prefix).await?;
        
        // 解析文件名提取 segment_id
        let mut segments = Vec::new();
        for file in files {
            if let Some(id) = self.parse_segment_id(&file) {
                segments.push(id);
            }
        }
        
        segments.sort_unstable();
        Ok(segments)
    }
    
    async fn cleanup_old_segments(
        &self,
        stream_id: &str,
        keep_count: usize,
    ) -> Result<usize> {
        let segments = self.list_segments(stream_id).await?;
        
        if segments.len() <= keep_count {
            return Ok(0);
        }
        
        let to_delete = &segments[..segments.len() - keep_count];
        let mut deleted = 0;
        
        for &segment_id in to_delete {
            if self.delete_segment(stream_id, segment_id).await.is_ok() {
                deleted += 1;
            }
        }
        
        Ok(deleted)
    }
}
```

---

## 4. HlsManager 使用方式

```rust
// flux-rtmpd/src/hls_manager.rs

impl HlsManager {
    pub fn with_storage_manager(
        storage_manager: Arc<StorageManager>,
        timeshift: Option<Arc<TimeShiftCore>>,
        telemetry: TelemetryClient,
    ) -> Self {
        // 创建 SegmentStorage（使用 StorageManager）
        let segment_storage = Arc::new(SegmentStorageImpl::new(storage_manager));
        
        Self::with_storage(segment_storage, timeshift, telemetry)
    }
}
```

---

## 5. 配置示例

### 5.1 本地存储

```rust
let storage_manager = Arc::new(StorageManager::new());

// 初始化本地后端
let local_backend = Arc::new(LocalBackend::new(PathBuf::from("/data/hls")));

let pool_config = PoolConfig {
    name: "local-pool".to_string(),
    priority: 1,
    max_usage_percent: 90.0,
};

storage_manager.initialize_with_backends(vec![
    (pool_config, local_backend),
]).await?;

// 创建 HlsManager
let hls_manager = HlsManager::with_storage_manager(
    storage_manager,
    None,
    TelemetryClient::new(None, 1000),
);
```

### 5.2 混合存储（本地 + S3）

```rust
let storage_manager = Arc::new(StorageManager::new());

// 本地存储池（高优先级）
let local_backend = Arc::new(LocalBackend::new(PathBuf::from("/data/hls")));
let local_config = PoolConfig {
    name: "local-ssd".to_string(),
    priority: 1,  // 高优先级
    max_usage_percent: 80.0,
};

// S3 存储池（低优先级，备份）
let s3_backend = Arc::new(S3Backend::new(
    "my-hls-bucket".to_string(),
    "us-west-2".to_string(),
).await?);
let s3_config = PoolConfig {
    name: "s3-backup".to_string(),
    priority: 2,  // 低优先级
    max_usage_percent: 100.0,
};

storage_manager.initialize_with_backends(vec![
    (local_config, local_backend),
    (s3_config, s3_backend),
]).await?;

// HlsManager 会自动选择最佳存储池
let hls_manager = HlsManager::with_storage_manager(
    storage_manager,
    None,
    TelemetryClient::new(None, 1000),
);
```

---

## 6. 架构优势

### 6.1 关注点分离

| 层次 | 关注点 |
|------|--------|
| `StorageBackend` | 如何读写（本地/S3/OSS） |
| `StorageManager` | 选择哪个池，负载均衡 |
| `SegmentStorage` | 分片路径构造，业务逻辑 |
| `HlsManager` | HLS 流程协调 |

### 6.2 易于扩展

添加新的存储后端只需：
1. 实现 `StorageBackend` trait
2. 在配置中添加新的池
3. 无需修改 `SegmentStorage` 和 `HlsManager`

### 6.3 灵活配置

```rust
// 场景 1：纯本地存储
storage_manager.add_pool(local_backend);

// 场景 2：本地 + S3 备份
storage_manager.add_pool(local_backend, priority=1);
storage_manager.add_pool(s3_backend, priority=2);

// 场景 3：多区域 S3
storage_manager.add_pool(s3_us_west, priority=1);
storage_manager.add_pool(s3_us_east, priority=2);
storage_manager.add_pool(s3_eu, priority=3);
```

---

## 7. 实施计划

### 阶段 1：基础设施层（flux-storage）
1. 定义 `StorageBackend` trait
2. 实现 `LocalBackend`
3. 扩展 `StoragePool` 支持 Backend
4. 扩展 `StorageManager` 支持多后端

### 阶段 2：业务抽象层
1. 重构 `SegmentStorage` 使用 `StorageManager`
2. 移除直接文件 I/O

### 阶段 3：扩展存储后端（可选）
1. 实现 `S3Backend`
2. 实现 `OSSBackend`
3. 实现 `RedisBackend`

---

## 8. 总结

**核心思想**：
- `StorageBackend` 解决"如何存储"（本地/S3/OSS）
- `StorageManager` 解决"存储到哪里"（负载均衡）
- `SegmentStorage` 解决"存储什么"（业务逻辑）
- `HlsManager` 解决"何时存储"（流程协调）

**每一层只做一件事，职责清晰，易于扩展！**
