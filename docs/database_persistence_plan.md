# flux-device 数据库持久化完善计划

> **版本**: v1.0  
> **日期**: 2026-02-22  
> **当前状态**: 仅使用内存缓存，数据库持久化未实现  
> **目标**: 完整实现数据库持久化功能

---

## 📋 目录

- [1. 当前状态分析](#1-当前状态分析)
- [2. 待实现功能清单](#2-待实现功能清单)
- [3. 实施计划](#3-实施计划)
- [4. 技术方案](#4-技术方案)
- [5. 测试计划](#5-测试计划)

---

## 1. 当前状态分析

### 1.1 已完成

✅ **数据库设计**：
- 4张表的完整 SQL 设计
- 索引优化
- 外键约束
- 触发器（自动更新时间戳）

✅ **SeaORM 实体**：
- `device::Entity` - 设备实体
- `device_group::Entity` - 分组实体
- `device_status_history::Entity` - 状态历史实体
- `device_metrics::Entity` - 指标实体
- 关系映射定义

✅ **内存缓存实现**：
- 设备注册表缓存
- 分组缓存
- 心跳时间记录

### 1.2 未实现（待完成）

❌ **数据库持久化**：
- 所有 CRUD 操作都只在内存中
- 没有实际的数据库读写
- 数据重启后丢失

❌ **缓存同步**：
- 缓存与数据库不一致
- 没有缓存失效策略
- 没有缓存预热

❌ **事务支持**：
- 没有事务管理
- 批量操作不是原子的

---

## 2. 待实现功能清单

### 2.1 DeviceRegistry（设备注册表）

#### 需要实现的数据库操作

**文件**: `src/registry.rs`

| 方法 | 当前状态 | TODO 位置 | 优先级 |
|------|---------|-----------|--------|
| `register()` | ❌ 仅缓存 | Line 68 | 🔥 高 |
| `unregister()` | ❌ 仅缓存 | Line 101 | 🔥 高 |
| `get()` | ❌ 仅缓存 | Line 123 | 🔥 高 |
| `update()` | ❌ 仅缓存 | Line 151 | 🔥 高 |
| `list()` | ❌ 仅缓存 | Line 174 | 🔥 高 |
| `exists()` | ❌ 仅缓存 | Line 203 | 🔥 高 |
| `count()` | ❌ 仅缓存 | Line 217 | 🟡 中 |
| `warm_cache()` | ❌ 未实现 | Line 237 | 🟡 中 |

**具体需要实现**：

1. **register() - 设备注册**
```rust
// TODO: 保存到数据库
// 需要实现：
// 1. 将 Device 模型转换为 device::ActiveModel
// 2. 使用 Entity::insert() 插入数据库
// 3. 处理唯一约束冲突
// 4. 同步更新缓存
```

2. **unregister() - 设备注销**
```rust
// TODO: 从数据库删除
// 需要实现：
// 1. 使用 Entity::delete_by_id() 删除
// 2. 级联删除相关数据（状态历史、指标）
// 3. 同步更新缓存
```

3. **get() - 获取设备**
```rust
// TODO: 从数据库查询
// 需要实现：
// 1. 先查缓存，未命中再查数据库
// 2. 使用 Entity::find_by_id() 查询
// 3. 将 device::Model 转换为 Device
// 4. 更新缓存
```

4. **update() - 更新设备**
```rust
// TODO: 更新到数据库
// 需要实现：
// 1. 将 Device 转换为 device::ActiveModel
// 2. 使用 Entity::update() 更新
// 3. 处理并发更新（乐观锁）
// 4. 同步更新缓存
```

5. **list() - 列出设备**
```rust
// TODO: 从数据库查询并应用过滤条件
// 需要实现：
// 1. 构建 SeaORM 查询条件
// 2. 应用过滤器（类型、协议、状态等）
// 3. 应用分页
// 4. 批量转换为 Device 模型
```

6. **exists() - 检查存在**
```rust
// TODO: 从数据库查询
// 需要实现：
// 1. 使用 Entity::find_by_id().count() 检查
// 2. 缓存结果
```

7. **count() - 统计数量**
```rust
// TODO: 从数据库统计
// 需要实现：
// 1. 构建查询条件
// 2. 使用 Entity::find().count() 统计
```

8. **warm_cache() - 缓存预热**
```rust
// TODO: 从数据库加载所有设备
// 需要实现：
// 1. 批量查询所有设备
// 2. 加载到缓存
// 3. 限制数量（避免内存溢出）
```

---

### 2.2 DeviceMonitor（设备监控）

#### 需要实现的数据库操作

**文件**: `src/monitor.rs`

| 功能 | 当前状态 | TODO 位置 | 优先级 |
|------|---------|-----------|--------|
| `get_metrics()` | ❌ 返回空 | Line 159 | 🔥 高 |
| `record_metric()` | ❌ 仅日志 | Line 177 | 🔥 高 |
| 状态历史记录 | ❌ 未实现 | - | 🟡 中 |

**具体需要实现**：

1. **record_metric() - 记录指标**
```rust
// TODO: 保存到时序数据库
// 需要实现：
// 1. 将指标数据转换为 device_metrics::ActiveModel
// 2. 批量插入优化（减少数据库压力）
// 3. 考虑使用 InfluxDB 替代 PostgreSQL
```

2. **get_metrics() - 获取指标**
```rust
// TODO: 从时序数据库查询指标
// 需要实现：
// 1. 按时间范围查询
// 2. 按指标名称过滤
// 3. 数据聚合（平均值、最大值、最小值）
// 4. 分页支持
```

3. **状态历史记录**
```rust
// 需要新增功能：
// 1. 在状态变更时自动记录到 device_status_history
// 2. 提供查询状态历史的方法
// 3. 状态变更事件通知
```

---

### 2.3 DeviceGroupManager（设备分组）

#### 需要实现的数据库操作

**文件**: `src/group.rs`

| 方法 | 当前状态 | TODO 位置 | 优先级 |
|------|---------|-----------|--------|
| `create_group()` | ❌ 仅缓存 | Line 73 | 🔥 高 |
| `get_group()` | ❌ 仅缓存 | Line 103 | 🔥 高 |
| `update_group()` | ❌ 仅缓存 | Line 129 | 🔥 高 |
| `delete_group()` | ❌ 仅缓存 | Line 165 | 🔥 高 |
| `list_groups()` | ❌ 仅缓存 | Line 182 | 🔥 高 |
| `get_children()` | ❌ 仅缓存 | Line 199 | 🟡 中 |

**具体需要实现**：

1. **create_group() - 创建分组**
```rust
// TODO: 保存到数据库
// 需要实现：
// 1. 转换为 device_group::ActiveModel
// 2. 插入数据库
// 3. 处理父分组关系
// 4. 同步缓存
```

2. **get_group() - 获取分组**
```rust
// TODO: 从数据库查询
// 需要实现：
// 1. 缓存优先
// 2. 数据库查询
// 3. 模型转换
```

3. **update_group() - 更新分组**
```rust
// TODO: 更新到数据库
// 需要实现：
// 1. 更新分组信息
// 2. 更新路径（如果父分组变更）
// 3. 级联更新子分组路径
```

4. **delete_group() - 删除分组**
```rust
// TODO: 从数据库删除
// 需要实现：
// 1. 检查约束（设备、子分组）
// 2. 级联删除或拒绝
// 3. 同步缓存
```

5. **list_groups() - 列出分组**
```rust
// TODO: 从数据库查询
// 需要实现：
// 1. 查询所有分组
// 2. 构建层级结构
// 3. 批量加载
```

6. **get_children() - 获取子分组**
```rust
// TODO: 从数据库查询
// 需要实现：
// 1. 按 parent_id 查询
// 2. 支持递归查询（所有后代）
```

---

### 2.4 数据模型转换

#### 需要实现的转换函数

**新增文件**: `src/db/converter.rs`

```rust
// Device <-> device::Model 转换
impl From<Device> for device::ActiveModel { }
impl From<device::Model> for Device { }

// DeviceGroup <-> device_group::Model 转换
impl From<DeviceGroup> for device_group::ActiveModel { }
impl From<device_group::Model> for DeviceGroup { }

// DeviceMetrics <-> device_metrics::Model 转换
impl From<DeviceMetrics> for device_metrics::ActiveModel { }
impl From<device_metrics::Model> for DeviceMetrics { }

// DeviceStatusHistory <-> device_status_history::Model 转换
impl From<DeviceStatusHistory> for device_status_history::ActiveModel { }
impl From<device_status_history::Model> for DeviceStatusHistory { }
```

**复杂度**：
- 需要处理 JSONB 字段（metadata、location）
- 需要处理数组字段（tags）
- 需要处理枚举类型转换（DeviceType、Protocol、DeviceStatus）
- 需要处理 Option 类型

---

### 2.5 缓存策略

#### 需要实现的缓存管理

**新增文件**: `src/cache.rs`

```rust
pub struct CacheStrategy {
    // 缓存失效策略
    ttl: Duration,              // 缓存生存时间
    max_size: usize,            // 最大缓存数量
    
    // 缓存更新策略
    write_through: bool,        // 写穿透（同步写数据库和缓存）
    write_back: bool,           // 写回（先写缓存，异步写数据库）
    
    // 缓存失效策略
    lru: bool,                  // LRU 淘汰
}

pub trait CacheManager {
    async fn get(&self, key: &str) -> Option<Device>;
    async fn set(&self, key: &str, value: Device);
    async fn invalidate(&self, key: &str);
    async fn clear(&self);
    async fn warm_up(&self);
}
```

**需要实现**：
1. ✅ 内存缓存（已实现）
2. ❌ Redis 缓存（可选）
3. ❌ 缓存失效策略
4. ❌ 缓存预热
5. ❌ 缓存一致性保证

---

### 2.6 事务支持

#### 需要实现的事务管理

**新增方法**：

```rust
impl DeviceRegistry {
    // 批量操作（事务）
    pub async fn register_batch(&self, devices: Vec<Device>) -> Result<Vec<Device>>;
    pub async fn delete_batch(&self, device_ids: &[String]) -> Result<()>;
}

impl DeviceGroupManager {
    // 移动分组（事务）
    pub async fn move_group_with_devices(&self, group_id: &str, new_parent: Option<String>) -> Result<()>;
}
```

**需要实现**：
1. ❌ 使用 SeaORM 事务 API
2. ❌ 批量操作原子性
3. ❌ 错误回滚
4. ❌ 并发控制（乐观锁/悲观锁）

---

## 3. 实施计划

### 阶段 1：基础持久化（1周）🔥

**目标**：实现核心 CRUD 的数据库持久化

**任务**：
1. **Day 1-2**: 数据模型转换
   - 创建 `converter.rs`
   - 实现 Device 转换
   - 实现 DeviceGroup 转换
   - 单元测试

2. **Day 3-4**: DeviceRegistry 持久化
   - 实现 `register()` 数据库操作
   - 实现 `get()` 数据库操作
   - 实现 `update()` 数据库操作
   - 实现 `unregister()` 数据库操作
   - 集成测试

3. **Day 5**: DeviceGroupManager 持久化
   - 实现 `create_group()` 数据库操作
   - 实现 `get_group()` 数据库操作
   - 实现 `update_group()` 数据库操作
   - 实现 `delete_group()` 数据库操作

4. **Day 6-7**: 查询和过滤
   - 实现 `list()` 数据库查询
   - 实现过滤条件构建
   - 实现分页
   - 性能优化

**交付物**：
- ✅ 数据模型转换器
- ✅ Registry 完整持久化
- ✅ GroupManager 完整持久化
- ✅ 集成测试通过

---

### 阶段 2：监控和指标（3-5天）🔥

**目标**：实现设备监控数据持久化

**任务**：
1. **Day 1-2**: 状态历史
   - 实现状态变更自动记录
   - 实现状态历史查询
   - 数据清理策略

2. **Day 3**: 设备指标
   - 实现 `record_metric()` 持久化
   - 实现 `get_metrics()` 查询
   - 批量插入优化

3. **Day 4-5**: 时序数据优化
   - 评估 InfluxDB 集成
   - 数据聚合查询
   - 性能测试

**交付物**：
- ✅ 状态历史记录
- ✅ 指标持久化
- ✅ 查询优化

---

### 阶段 3：缓存优化（3-5天）🟡

**目标**：完善缓存策略，提高性能

**任务**：
1. **Day 1-2**: 缓存策略
   - 实现缓存失效策略
   - 实现 LRU 淘汰
   - 实现缓存预热

2. **Day 3**: Redis 集成（可选）
   - Redis 连接池
   - Redis 缓存实现
   - 缓存一致性

3. **Day 4-5**: 性能优化
   - 查询优化
   - 批量操作优化
   - 并发测试

**交付物**：
- ✅ 完善的缓存策略
- ✅ Redis 集成（可选）
- ✅ 性能基准测试

---

### 阶段 4：事务和高级功能（2-3天）🟡

**目标**：实现事务支持和高级功能

**任务**：
1. **Day 1**: 事务支持
   - 批量操作事务
   - 错误回滚
   - 并发控制

2. **Day 2**: 高级查询
   - 复杂过滤条件
   - 聚合查询
   - 全文搜索（可选）

3. **Day 3**: 数据迁移
   - 数据导入导出
   - 版本迁移
   - 数据备份

**交付物**：
- ✅ 事务支持
- ✅ 高级查询
- ✅ 数据迁移工具

---

## 4. 技术方案

### 4.1 SeaORM 使用

#### 基本操作示例

```rust
use sea_orm::*;

// 插入
let device = device::ActiveModel {
    id: Set(device.id.clone()),
    name: Set(device.name.clone()),
    device_type: Set(device.device_type.as_str().to_string()),
    // ...
};
let result = device::Entity::insert(device).exec(&self.db).await?;

// 查询
let device = device::Entity::find_by_id(device_id)
    .one(&self.db)
    .await?;

// 更新
let mut device: device::ActiveModel = device.into();
device.name = Set("新名称".to_string());
device.update(&self.db).await?;

// 删除
device::Entity::delete_by_id(device_id)
    .exec(&self.db)
    .await?;

// 查询列表（带过滤）
let devices = device::Entity::find()
    .filter(device::Column::DeviceType.eq("Sensor"))
    .filter(device::Column::Status.eq("Online"))
    .order_by_desc(device::Column::CreatedAt)
    .paginate(&self.db, page_size)
    .fetch_page(page)
    .await?;
```

#### 事务示例

```rust
let txn = self.db.begin().await?;

// 操作1
device::Entity::insert(device1).exec(&txn).await?;

// 操作2
device::Entity::insert(device2).exec(&txn).await?;

// 提交或回滚
txn.commit().await?;
// 或 txn.rollback().await?;
```

---

### 4.2 数据转换策略

#### JSONB 字段处理

```rust
// metadata: HashMap<String, String> -> JSONB
let metadata_json = serde_json::to_value(&device.metadata)?;

// JSONB -> HashMap<String, String>
let metadata: HashMap<String, String> = serde_json::from_value(model.metadata)?;
```

#### 数组字段处理

```rust
// PostgreSQL 数组
tags: Vec<String> -> TEXT[]
```

#### 枚举转换

```rust
impl DeviceType {
    pub fn to_db_string(&self) -> String {
        self.as_str().to_string()
    }
    
    pub fn from_db_string(s: &str) -> Self {
        Self::from_str(s)
    }
}
```

---

### 4.3 缓存策略

#### 写穿透（Write-Through）

```rust
pub async fn register(&self, device: Device) -> Result<Device> {
    // 1. 写数据库
    let model = self.insert_to_db(&device).await?;
    
    // 2. 写缓存
    self.cache.set(&device.id, device.clone()).await;
    
    Ok(device)
}
```

#### 缓存失效

```rust
pub async fn update(&self, device_id: &str, device: Device) -> Result<Device> {
    // 1. 更新数据库
    let model = self.update_to_db(device_id, &device).await?;
    
    // 2. 失效缓存
    self.cache.invalidate(device_id).await;
    
    Ok(device)
}
```

---

### 4.4 性能优化

#### 批量插入

```rust
// 使用批量插入减少数据库往返
let devices: Vec<device::ActiveModel> = devices.into_iter()
    .map(|d| d.into())
    .collect();

device::Entity::insert_many(devices)
    .exec(&self.db)
    .await?;
```

#### 预加载关联

```rust
// 使用 JOIN 减少查询次数
let devices = device::Entity::find()
    .find_also_related(device_group::Entity)
    .all(&self.db)
    .await?;
```

#### 索引优化

```sql
-- 确保关键字段有索引
CREATE INDEX idx_devices_status ON devices(status);
CREATE INDEX idx_devices_type ON devices(device_type);
CREATE INDEX idx_devices_group ON devices(group_id);
```

---

## 5. 测试计划

### 5.1 单元测试

```rust
#[tokio::test]
async fn test_device_persistence() {
    let db = setup_test_db().await;
    let registry = DeviceRegistry::new(db);
    
    // 测试插入
    let device = create_test_device();
    registry.register(device.clone()).await.unwrap();
    
    // 测试查询
    let found = registry.get(&device.id).await.unwrap();
    assert!(found.is_some());
    
    // 测试更新
    let mut updated = found.unwrap();
    updated.name = "新名称".to_string();
    registry.update(&device.id, updated).await.unwrap();
    
    // 测试删除
    registry.unregister(&device.id).await.unwrap();
    let deleted = registry.get(&device.id).await.unwrap();
    assert!(deleted.is_none());
}
```

### 5.2 集成测试

```rust
#[tokio::test]
async fn test_device_lifecycle_with_db() {
    // 完整的设备生命周期测试
    // 包括：注册、心跳、分组、指标、删除
}
```

### 5.3 性能测试

```rust
#[tokio::test]
async fn test_bulk_insert_performance() {
    // 测试批量插入1000个设备的性能
    let start = Instant::now();
    
    for i in 0..1000 {
        // 插入设备
    }
    
    let duration = start.elapsed();
    assert!(duration < Duration::from_secs(10));
}
```

---

## 6. 风险和挑战

### 6.1 技术风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 数据库性能瓶颈 | 高 | 索引优化、批量操作、缓存 |
| 缓存一致性问题 | 中 | 写穿透策略、缓存失效 |
| 并发冲突 | 中 | 乐观锁、事务隔离 |
| 数据迁移复杂 | 低 | 版本管理、测试 |

### 6.2 实施挑战

- **时间估算**: 完整实现需要 2-3 周
- **测试覆盖**: 需要大量集成测试
- **向后兼容**: 需要保持 API 兼容性
- **性能要求**: 需要达到生产级性能

---

## 7. 验收标准

### 7.1 功能完整性

- ✅ 所有 TODO 已实现
- ✅ 数据持久化到数据库
- ✅ 缓存与数据库同步
- ✅ 事务支持

### 7.2 性能指标

- ✅ 设备注册 < 10ms
- ✅ 设备查询 < 5ms
- ✅ 批量操作（100设备）< 100ms
- ✅ 并发支持 > 1000 QPS

### 7.3 质量标准

- ✅ 测试覆盖率 > 80%
- ✅ 无数据丢失
- ✅ 缓存命中率 > 90%
- ✅ 错误处理完善

---

## 8. 总结

### 当前缺失

**核心功能**（18个 TODO）：
- ❌ DeviceRegistry: 8个数据库操作
- ❌ DeviceMonitor: 2个数据库操作
- ❌ DeviceGroupManager: 6个数据库操作
- ❌ 数据模型转换: 4组转换函数
- ❌ 缓存策略: 完整实现
- ❌ 事务支持: 批量操作

### 实施优先级

**P0 - 必须完成**（1-2周）：
1. 数据模型转换
2. DeviceRegistry 持久化
3. DeviceGroupManager 持久化
4. 基本测试

**P1 - 重要功能**（3-5天）：
5. 状态历史记录
6. 指标持久化
7. 查询优化

**P2 - 优化功能**（3-5天）：
8. 缓存策略完善
9. Redis 集成
10. 事务支持

### 预计工期

- **最小可用版本**: 1-2 周
- **完整功能版本**: 2-3 周
- **生产就绪版本**: 3-4 周

---

**维护者**: FLUX IOT Team  
**创建日期**: 2026-02-22  
**最后更新**: 2026-02-22
