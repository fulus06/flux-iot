#[cfg(feature = "postgres")]
use flux_storage::{LocalSegmentStorage, PostgresMetadataBackend, SegmentMetadata, SegmentStorage};
#[cfg(feature = "postgres")]
use std::collections::HashMap;
#[cfg(feature = "postgres")]
use std::path::PathBuf;
#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 混合模式元数据存储示例 ===\n");
    println!("内存缓存 + PostgreSQL 持久化\n");

    // 1. 创建 PostgreSQL 后端
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/flux_iot".to_string());
    
    println!("1. 连接到 PostgreSQL: {}", database_url);
    let pg_backend = Arc::new(PostgresMetadataBackend::from_url(&database_url).await?);
    
    // 运行迁移
    pg_backend.run_migrations().await?;
    println!("   ✅ 数据库迁移完成\n");

    // 2. 创建混合模式存储
    println!("2. 创建混合模式存储（内存 + PostgreSQL）");
    let storage = LocalSegmentStorage::with_postgres(
        PathBuf::from("./data/hybrid"),
        Some(pg_backend.clone()),
    );
    println!("   ✅ 混合模式存储已创建\n");

    // 3. 保存分片（带元数据）
    println!("3. 保存分片（write-through 模式）");
    
    let mut metadata = SegmentMetadata::new();
    metadata
        .set("start_time", "2026-02-23T16:00:00Z")
        .set("duration", "10.0")
        .set("has_keyframe", "true")
        .set("codec", "h264")
        .set("resolution", "1920x1080");
    
    let data = b"video segment data...";
    storage.save_segment_with_metadata(
        "app/stream1",
        1,
        metadata,
        data,
    ).await?;
    
    println!("   ✅ 数据保存到文件系统");
    println!("   ✅ 元数据保存到内存缓存");
    println!("   ✅ 元数据异步同步到 PostgreSQL\n");

    // 等待异步写入完成
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 4. 从缓存读取（快速）
    println!("4. 从内存缓存读取（第一次）");
    let start = std::time::Instant::now();
    let metadata = storage.get_segment_metadata("app/stream1", 1).await?;
    let duration = start.elapsed();
    println!("   ✅ 读取成功");
    println!("   ⏱️  耗时: {:?} (内存缓存)", duration);
    println!("   📊 元数据: codec={}", metadata.get("codec").unwrap_or(&"N/A".to_string()));
    println!();

    // 5. 清空内存缓存，测试从 PostgreSQL 读取
    println!("5. 清空内存缓存，从 PostgreSQL 读取");
    // 这里我们创建一个新的存储实例来模拟缓存清空
    let storage2 = LocalSegmentStorage::with_postgres(
        PathBuf::from("./data/hybrid"),
        Some(pg_backend.clone()),
    );
    
    let start = std::time::Instant::now();
    let metadata = storage2.get_segment_metadata("app/stream1", 1).await?;
    let duration = start.elapsed();
    println!("   ✅ 从 PostgreSQL 读取成功");
    println!("   ⏱️  耗时: {:?} (PostgreSQL + 缓存更新)", duration);
    println!("   📊 元数据: codec={}", metadata.get("codec").unwrap_or(&"N/A".to_string()));
    println!();

    // 6. 再次读取（应该从缓存）
    println!("6. 再次读取（应该从缓存）");
    let start = std::time::Instant::now();
    let metadata = storage2.get_segment_metadata("app/stream1", 1).await?;
    let duration = start.elapsed();
    println!("   ✅ 读取成功");
    println!("   ⏱️  耗时: {:?} (内存缓存)", duration);
    println!();

    // 7. 保存更多分片
    println!("7. 保存更多分片");
    for i in 2..=5 {
        let mut meta = SegmentMetadata::new();
        meta.set("start_time", format!("2026-02-23T16:00:{:02}Z", i * 10))
            .set("duration", "10.0")
            .set("has_keyframe", if i % 3 == 0 { "true" } else { "false" })
            .set("codec", "h264");
        
        storage.save_segment_with_metadata("app/stream1", i, meta, data).await?;
    }
    println!("   ✅ 已保存 5 个分片\n");

    // 等待异步写入
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // 8. 查询元数据（使用 PostgreSQL 的强大查询）
    println!("8. 查询关键帧（使用 PostgreSQL JSONB 查询）");
    
    let mut filter = HashMap::new();
    filter.insert("has_keyframe".to_string(), "true".to_string());
    
    let start = std::time::Instant::now();
    let results = storage.query_metadata("app/stream1", filter).await?;
    let duration = start.elapsed();
    
    println!("   ✅ 找到 {} 个关键帧分片", results.len());
    println!("   ⏱️  查询耗时: {:?}", duration);
    for (segment_id, metadata) in &results {
        println!("     - 分片 {}: start_time={}", 
            segment_id, 
            metadata.get("start_time").unwrap_or(&"N/A".to_string())
        );
    }
    println!();

    // 9. 验证 PostgreSQL 中的数据
    println!("9. 直接从 PostgreSQL 验证数据");
    let pg_results = pg_backend.query_metadata(
        "app/stream1",
        HashMap::new(), // 无过滤，获取所有
    ).await?;
    println!("   ✅ PostgreSQL 中有 {} 条元数据记录", pg_results.len());
    println!();

    println!("=== 示例完成 ===\n");
    println!("💡 混合模式特性:");
    println!("   ✅ 内存缓存提供极快的读取速度");
    println!("   ✅ PostgreSQL 提供持久化存储");
    println!("   ✅ Write-through 策略保证数据一致性");
    println!("   ✅ Cache-aside 模式优化读取性能");
    println!("   ✅ PostgreSQL JSONB 提供强大查询能力");
    println!("   ✅ 自动故障转移（PostgreSQL 不可用时使用内存）");

    Ok(())
}

#[cfg(not(feature = "postgres"))]
fn main() {
    println!("此示例需要启用 postgres feature");
    println!("运行: cargo run --example hybrid_metadata --features postgres");
}
