use flux_storage::{LocalSegmentStorage, SegmentMetadata, SegmentStorage};
use std::collections::HashMap;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== flux-storage 元数据使用示例 ===\n");

    let storage = LocalSegmentStorage::new(PathBuf::from("./data/example"));

    // 示例 1: 保存分片（带自定义元数据）
    println!("1. 保存分片（带元数据）");
    
    let mut metadata = SegmentMetadata::new();
    metadata
        .set("start_time", "2026-02-23T15:00:00Z")
        .set("duration", "10.0")
        .set("size", "1024000")
        .set("has_keyframe", "true")
        .set("codec", "h264")
        .set("resolution", "1920x1080")
        .set("bitrate", "2000000");
    
    let data = b"video segment data...";
    storage.save_segment_with_metadata(
        "app/stream1",
        1,
        metadata,
        data,
    ).await?;
    
    println!("   ✅ 分片已保存（带元数据）\n");

    // 示例 2: 保存更多分片
    println!("2. 保存更多分片");
    
    for i in 2..=5 {
        let mut meta = SegmentMetadata::new();
        meta.set("start_time", format!("2026-02-23T15:00:{:02}Z", i * 10))
            .set("duration", "10.0")
            .set("has_keyframe", if i % 3 == 0 { "true" } else { "false" })
            .set("codec", "h264");
        
        storage.save_segment_with_metadata(
            "app/stream1",
            i,
            meta,
            data,
        ).await?;
    }
    
    println!("   ✅ 已保存 5 个分片\n");

    // 示例 3: 获取单个分片的元数据
    println!("3. 获取分片元数据");
    
    let metadata = storage.get_segment_metadata("app/stream1", 1).await?;
    println!("   分片 1 的元数据:");
    for (key, value) in &metadata.metadata {
        println!("     - {}: {}", key, value);
    }
    println!();

    // 示例 4: 查询元数据（查找所有关键帧）
    println!("4. 查询关键帧分片");
    
    let mut filter = HashMap::new();
    filter.insert("has_keyframe".to_string(), "true".to_string());
    
    let results = storage.query_metadata("app/stream1", filter).await?;
    println!("   找到 {} 个关键帧分片:", results.len());
    for (segment_id, metadata) in &results {
        println!("     - 分片 {}: start_time={}", 
            segment_id, 
            metadata.get("start_time").unwrap_or(&"N/A".to_string())
        );
    }
    println!();

    // 示例 5: 查询元数据（查找 h264 编码的分片）
    println!("5. 查询 h264 编码的分片");
    
    let mut filter = HashMap::new();
    filter.insert("codec".to_string(), "h264".to_string());
    
    let results = storage.query_metadata("app/stream1", filter).await?;
    println!("   找到 {} 个 h264 分片", results.len());
    println!();

    // 示例 6: 复杂查询（h264 + 关键帧）
    println!("6. 复杂查询（h264 + 关键帧）");
    
    let mut filter = HashMap::new();
    filter.insert("codec".to_string(), "h264".to_string());
    filter.insert("has_keyframe".to_string(), "true".to_string());
    
    let results = storage.query_metadata("app/stream1", filter).await?;
    println!("   找到 {} 个符合条件的分片", results.len());
    println!();

    println!("=== 示例完成 ===\n");
    println!("💡 元数据特性:");
    println!("   ✅ 完全自定义的 key-value 结构");
    println!("   ✅ 由调用方决定元数据内容");
    println!("   ✅ 支持灵活的查询过滤");
    println!("   ✅ 类似 OSS 的对象元数据");

    Ok(())
}
