# FLUX IOT 平台测试指南

## 目录
- [测试架构](#测试架构)
- [测试分层](#测试分层)
- [测试工具链](#测试工具链)
- [编写测试用例](#编写测试用例)
- [运行测试](#运行测试)
- [CI/CD 集成](#cicd-集成)
- [测试覆盖率](#测试覆盖率)

---

## 测试架构

FLUX IOT 采用**金字塔测试模型**，从下至上分为：

```
        ┌─────────────────┐
        │   E2E Tests     │  端到端测试（少量，关键场景）
        │   (5%)          │
        ├─────────────────┤
        │ Integration     │  集成测试（中等，模块协作）
        │ Tests (25%)     │
        ├─────────────────┤
        │   Unit Tests    │  单元测试（大量，细粒度）
        │   (70%)         │
        └─────────────────┘
```

---

## 测试分层

### 1. 单元测试 (Unit Tests)

**目标**：测试单个函数、结构体方法的正确性

**位置**：`src/` 目录下，与源码同文件
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_function_name() {
        // 测试逻辑
    }
}
```

**覆盖范围**：
- ✅ 核心算法逻辑
- ✅ 数据结构操作
- ✅ 错误处理路径
- ✅ 边界条件
- ✅ 协议解析器

**示例模块**：
- `flux-core/src/bus.rs` - EventBus 发布/订阅
- `flux-video/src/codec.rs` - H.264 解析器
- `flux-mqtt/src/topic_matcher.rs` - MQTT 主题匹配
- `flux-storage/src/pool.rs` - 存储池管理

---

### 2. 集成测试 (Integration Tests)

**目标**：测试多个模块协作、跨边界交互

**位置**：`tests/` 目录（独立于 `src/`）
```rust
// tests/integration_test.rs
use my_crate::{ModuleA, ModuleB};

#[tokio::test]
async fn test_module_interaction() {
    // 测试 ModuleA 和 ModuleB 的协作
}
```

**覆盖范围**：
- ✅ API 端点（HTTP/WebSocket）
- ✅ 数据库操作（CRUD）
- ✅ 消息队列（MQTT Pub/Sub）
- ✅ 流媒体管道（RTSP → 存储）
- ✅ 配置热重载
- ✅ 插件加载与执行

**示例测试**：
- `flux-server/tests/api_tests.rs` - REST API 集成
- `flux-mqtt/tests/integration_test.rs` - MQTT Broker 功能
- `flux-video/tests/integration_test.rs` - 视频流处理管道

---

### 3. 端到端测试 (E2E Tests)

**目标**：模拟真实用户场景，验证完整业务流程

**位置**：`tests/e2e/` 或独立仓库

**覆盖范围**：
- ✅ 设备接入 → 数据上报 → 规则触发 → 通知
- ✅ GB28181 设备注册 → 实时预览 → 录像回放
- ✅ RTMP 推流 → HLS 转码 → 多码率播放
- ✅ 配置变更 → 服务热重载 → 无中断运行

---

## 测试工具链

### Rust 测试框架
```toml
[dev-dependencies]
tokio-test = "0.4"      # 异步测试工具
tempfile = "3"          # 临时文件/目录
mockall = "0.12"        # Mock 框架
wiremock = "0.6"        # HTTP Mock 服务器
criterion = "0.5"       # 性能基准测试
proptest = "1.0"        # 属性测试（模糊测试）
```

### 协议测试工具
- **MQTT**: `rumqttc` 客户端
- **RTSP**: `retina` 客户端
- **GB28181**: 自定义 SIP 客户端模拟器
- **HTTP**: `reqwest` + `axum::test`

### 性能测试
- **压测**: `wrk`, `hey`, `k6`
- **Profiling**: `cargo flamegraph`, `perf`
- **内存检测**: `valgrind`, `heaptrack`

---

## 编写测试用例

### 单元测试模板

```rust
// src/my_module.rs
pub struct MyService {
    config: Config,
}

impl MyService {
    pub fn process(&self, input: &str) -> Result<String, Error> {
        // 业务逻辑
        if input.is_empty() {
            return Err(Error::EmptyInput);
        }
        Ok(input.to_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_success() {
        let service = MyService { config: Config::default() };
        let result = service.process("hello");
        assert_eq!(result.unwrap(), "HELLO");
    }

    #[test]
    fn test_process_empty_input() {
        let service = MyService { config: Config::default() };
        let result = service.process("");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::EmptyInput));
    }
}
```

---

### 异步集成测试模板

```rust
// tests/integration_test.rs
use flux_server::{AppState, create_router};
use axum::{body::Body, http::Request};
use tower::ServiceExt;
use sea_orm::Database;

async fn setup_test_env() -> AppState {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    // 初始化其他组件...
    AppState { db, /* ... */ }
}

#[tokio::test]
async fn test_api_endpoint() {
    let state = setup_test_env().await;
    let app = create_router(Arc::new(state));

    let request = Request::builder()
        .uri("/api/v1/devices")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

---

### Mock 依赖模板

```rust
use mockall::mock;

// 定义 trait
#[async_trait]
pub trait DeviceRepository {
    async fn find_by_id(&self, id: &str) -> Result<Device, Error>;
}

// 生成 Mock
mock! {
    pub DeviceRepo {}
    
    #[async_trait]
    impl DeviceRepository for DeviceRepo {
        async fn find_by_id(&self, id: &str) -> Result<Device, Error>;
    }
}

#[tokio::test]
async fn test_with_mock() {
    let mut mock_repo = MockDeviceRepo::new();
    mock_repo
        .expect_find_by_id()
        .with(eq("device_123"))
        .returning(|_| Ok(Device { id: "device_123".into(), /* ... */ }));

    // 使用 mock_repo 进行测试
}
```

---

## 运行测试

### 基础命令

```bash
# 运行所有测试
cargo test

# 运行特定包的测试
cargo test -p flux-server

# 运行特定测试
cargo test test_api_endpoint

# 显示测试输出
cargo test -- --nocapture

# 并行度控制
cargo test -- --test-threads=1
```

### 异步测试

```bash
# 使用 tokio 运行时
cargo test --features tokio-test

# 设置超时
RUST_TEST_TIME_UNIT=5000 cargo test
```

### 集成测试

```bash
# 仅运行集成测试
cargo test --test integration_test

# 跳过单元测试
cargo test --tests
```

### 覆盖率报告

```bash
# 安装 tarpaulin
cargo install cargo-tarpaulin

# 生成覆盖率报告
cargo tarpaulin --out Html --output-dir coverage

# 指定包
cargo tarpaulin -p flux-server -p flux-mqtt --out Lcov
```

---

## CI/CD 集成

### GitHub Actions 配置

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run unit tests
        run: cargo test --lib --all-features
      
      - name: Run integration tests
        run: cargo test --test '*' --all-features
        env:
          DATABASE_URL: postgres://postgres:test@localhost/flux_test
      
      - name: Generate coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml --all-features
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: ./cobertura.xml
```

---

## 测试覆盖率

### 目标指标

| 模块类型 | 覆盖率目标 | 优先级 |
|---------|-----------|--------|
| 核心业务逻辑 | ≥ 90% | 🔴 高 |
| API 端点 | ≥ 85% | 🔴 高 |
| 协议解析器 | ≥ 95% | 🔴 高 |
| 存储层 | ≥ 80% | 🟡 中 |
| 工具函数 | ≥ 70% | 🟢 低 |

### 检查覆盖率

```bash
# 生成 HTML 报告
cargo tarpaulin --out Html

# 查看未覆盖的行
cargo tarpaulin --ignore-tests --out Stdout | grep "Uncovered Lines"

# 按模块查看
cargo tarpaulin --packages flux-server flux-mqtt --out Stdout
```

---

## 测试最佳实践

### ✅ DO（推荐）

1. **测试命名清晰**
   ```rust
   #[test]
   fn test_device_registration_with_valid_token() { }
   
   #[test]
   fn test_device_registration_fails_with_invalid_token() { }
   ```

2. **使用 AAA 模式**（Arrange-Act-Assert）
   ```rust
   #[test]
   fn test_example() {
       // Arrange: 准备测试数据
       let input = "test";
       
       // Act: 执行被测试的操作
       let result = process(input);
       
       // Assert: 验证结果
       assert_eq!(result, "expected");
   }
   ```

3. **测试边界条件**
   ```rust
   #[test]
   fn test_empty_input() { }
   
   #[test]
   fn test_max_length_input() { }
   
   #[test]
   fn test_special_characters() { }
   ```

4. **使用 `Result` 简化错误处理**
   ```rust
   #[tokio::test]
   async fn test_database_query() -> anyhow::Result<()> {
       let db = setup_db().await?;
       let result = db.query("SELECT 1").await?;
       assert_eq!(result, 1);
       Ok(())
   }
   ```

5. **清理测试资源**
   ```rust
   #[tokio::test]
   async fn test_with_cleanup() {
       let temp_dir = TempDir::new().unwrap();
       // 测试逻辑...
       // temp_dir 会在作用域结束时自动删除
   }
   ```

---

### ❌ DON'T（避免）

1. **不要依赖测试执行顺序**
   ```rust
   // ❌ 错误：依赖全局状态
   static mut COUNTER: i32 = 0;
   
   #[test]
   fn test_1() { unsafe { COUNTER += 1; } }
   
   #[test]
   fn test_2() { unsafe { assert_eq!(COUNTER, 1); } } // 可能失败
   ```

2. **不要使用 `unwrap()` 在生产代码中**
   ```rust
   // ❌ 错误
   pub fn process(input: &str) -> String {
       input.parse::<i32>().unwrap().to_string()
   }
   
   // ✅ 正确
   pub fn process(input: &str) -> Result<String, ParseError> {
       Ok(input.parse::<i32>()?.to_string())
   }
   ```

3. **不要忽略异步测试的超时**
   ```rust
   // ✅ 使用 timeout 防止测试挂起
   #[tokio::test]
   async fn test_with_timeout() {
       let result = tokio::time::timeout(
           Duration::from_secs(5),
           long_running_task()
       ).await;
       assert!(result.is_ok());
   }
   ```

4. **不要在测试中硬编码绝对路径**
   ```rust
   // ❌ 错误
   let path = "/Users/alice/test.db";
   
   // ✅ 正确
   let temp_dir = TempDir::new().unwrap();
   let path = temp_dir.path().join("test.db");
   ```

---

## 附录：测试清单

### 核心模块测试清单

- [ ] **flux-core**
  - [ ] EventBus 发布/订阅
  - [ ] 并发订阅者处理
  - [ ] 消息过滤
  - [ ] 背压处理

- [ ] **flux-mqtt**
  - [ ] CONNECT/DISCONNECT
  - [ ] PUBLISH/SUBSCRIBE
  - [ ] QoS 0/1/2
  - [ ] Retained 消息
  - [ ] Will 消息
  - [ ] 主题通配符匹配

- [ ] **flux-video**
  - [ ] GB28181 SIP 注册
  - [ ] RTP 接收与解包
  - [ ] H.264/H.265 解析
  - [ ] 关键帧提取
  - [ ] 录像存储

- [ ] **flux-storage**
  - [ ] 多池管理
  - [ ] 磁盘健康检查
  - [ ] 自动故障转移
  - [ ] 容量监控
  - [x] 元数据索引（PostgreSQL）
  - [x] 混合缓存模式
  - [x] 通用 key-value 元数据

- [ ] **flux-server**
  - [ ] REST API 端点
  - [ ] 认证/授权
  - [ ] 配置热重载
  - [ ] 优雅关闭

- [x] **HLS 时移回放**
  - [x] 元数据记录
  - [x] 时移回放 API
  - [x] M3U8 生成
  - [x] 时间范围查询
  - [x] 关键帧过滤

---

## HLS 时移回放测试

### 自动化测试脚本

**位置**: `scripts/test_timeshift.sh`

**功能**:
- ✅ 推送测试 RTMP 流
- ✅ 验证元数据保存
- ✅ 测试实时播放
- ✅ 测试时移回放
- ✅ 测试分片加载

**运行方式**:
```bash
# 设置数据库 URL
export DATABASE_URL="postgres://localhost/flux_iot"

# 运行测试脚本
./scripts/test_timeshift.sh
```

### 手动测试步骤

#### 1. 启动服务

```bash
# 启动 flux-rtmpd（带 PostgreSQL 支持）
export DATABASE_URL="postgres://localhost/flux_iot"
cargo run -p flux-rtmpd --features postgres
```

#### 2. 推送测试流

```bash
# 使用 FFmpeg 推送测试流
ffmpeg -re -i test.mp4 -t 60 \
  -c:v libx264 -c:a aac \
  -f flv rtmp://localhost:1935/live/test123
```

#### 3. 验证元数据

```bash
# 查询 PostgreSQL 元数据
psql $DATABASE_URL -c "
SELECT 
    segment_id,
    metadata->>'start_time' as start_time,
    metadata->>'duration' as duration,
    metadata->>'size' as size
FROM storage.segment_metadata
WHERE stream_id = 'rtmp/live/test123'
  AND metadata->>'protocol' = 'hls'
ORDER BY segment_id DESC
LIMIT 10;
"
```

#### 4. 测试实时播放

```bash
# 获取实时播放列表
curl http://localhost:8082/hls/rtmp/live/test123/index.m3u8
```

#### 5. 测试时移回放

```bash
# 获取第一个分片的时间
FIRST_TIME=$(psql $DATABASE_URL -t -c "
SELECT metadata->>'start_time'
FROM storage.segment_metadata
WHERE stream_id = 'rtmp/live/test123'
ORDER BY segment_id LIMIT 1;
" | tr -d ' ')

# 测试时移回放
curl "http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=${FIRST_TIME}"

# 测试带时长参数
curl "http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=${FIRST_TIME}&duration=60"

# 测试关键帧参数
curl "http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=${FIRST_TIME}&from_keyframe=true"
```

#### 6. 测试分片加载

```bash
# 加载特定分片
curl -I http://localhost:8082/hls/rtmp/live/test123/segment_0.ts
```

### 性能测试

#### 元数据查询性能

```bash
# 使用 psql 的 \timing 命令
psql $DATABASE_URL << EOF
\timing on
SELECT COUNT(*)
FROM storage.segment_metadata
WHERE stream_id = 'rtmp/live/test123'
  AND metadata->>'protocol' = 'hls';
EOF
```

**预期结果**: < 5ms

#### 时移回放 API 性能

```bash
# 使用 curl 测量响应时间
curl -w "@curl-format.txt" -o /dev/null -s \
  "http://localhost:8082/hls/live/test123/timeshift.m3u8?start_time=2026-02-23T15:00:00Z"
```

**curl-format.txt**:
```
time_namelookup:  %{time_namelookup}\n
time_connect:  %{time_connect}\n
time_starttransfer:  %{time_starttransfer}\n
time_total:  %{time_total}\n
```

**预期结果**: < 50ms

### 集成测试

```rust
// tests/timeshift_integration_test.rs
use flux_storage::{LocalSegmentStorage, SegmentMetadata, SegmentStorage};
use std::collections::HashMap;

#[tokio::test]
async fn test_timeshift_metadata_query() -> anyhow::Result<()> {
    // 1. 创建测试存储
    let storage = LocalSegmentStorage::new(
        std::path::PathBuf::from("./test_data")
    );
    
    // 2. 保存测试分片
    let mut metadata = SegmentMetadata::new();
    metadata
        .set("protocol", "hls")
        .set("start_time", "2026-02-23T15:00:00Z")
        .set("duration", "10.0")
        .set("has_keyframe", "true");
    
    storage.save_segment_with_metadata(
        "test/stream",
        1,
        metadata,
        b"test data",
    ).await?;
    
    // 3. 查询元数据
    let mut filter = HashMap::new();
    filter.insert("protocol".to_string(), "hls".to_string());
    
    let results = storage.query_metadata("test/stream", filter).await?;
    
    // 4. 验证结果
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, 1);
    
    Ok(())
}
```

---

## 参考资料

- [Rust 测试官方文档](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio 测试指南](https://tokio.rs/tokio/topics/testing)
- [Property-based Testing in Rust](https://github.com/proptest-rs/proptest)
- [Rust API Guidelines - Testing](https://rust-lang.github.io/api-guidelines/documentation.html#examples-use-not-try-not-unwrap-c-question-mark)
