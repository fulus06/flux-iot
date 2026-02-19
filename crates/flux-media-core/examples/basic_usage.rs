use bytes::Bytes;
use chrono::Utc;
use flux_media_core::{
    snapshot::{SnapshotMode, SnapshotOrchestrator, SnapshotRequest, StubDecoder},
    storage::{filesystem::FileSystemStorage, MediaStorage, StorageConfig},
    types::StreamId,
};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== flux-media-core 基础使用示例 ===\n");

    // 1. 创建临时目录
    let temp_dir = tempdir()?;
    let storage_dir = temp_dir.path().join("storage");
    let keyframe_dir = temp_dir.path().join("keyframes");

    println!("存储目录: {:?}", storage_dir);
    println!("关键帧目录: {:?}\n", keyframe_dir);

    // 2. 初始化存储
    let config = StorageConfig {
        root_dir: storage_dir.clone(),
        retention_days: 7,
        segment_duration_secs: 60,
    };

    let mut storage = FileSystemStorage::new(config)?;
    println!("✓ 存储初始化完成");

    // 3. 初始化 Snapshot 编排器
    let orchestrator =
        SnapshotOrchestrator::new(keyframe_dir.clone()).with_decoder(Arc::new(StubDecoder));
    println!("✓ Snapshot 编排器初始化完成\n");

    // 4. 模拟 GB28181 流
    let stream_id = StreamId::new("gb28181", "34020000001320000001/34020000001320000001");
    println!("流 ID: {}\n", stream_id);

    // 5. 存储视频数据
    println!("--- 存储视频片段 ---");
    for i in 0..3 {
        let timestamp = Utc::now() + chrono::Duration::seconds(i);
        let data = Bytes::from(format!("video segment {}", i));

        storage.put_object(&stream_id, timestamp, data).await?;
        println!("✓ 存储片段 {} (时间戳: {})", i, timestamp);
    }

    // 6. 处理关键帧
    println!("\n--- 提取关键帧 ---");
    let mut h264_keyframe = Vec::new();
    h264_keyframe.extend_from_slice(&[0, 0, 0, 1, 0x67]); // SPS
    h264_keyframe.extend_from_slice(&[0, 0, 0, 1, 0x68]); // PPS
    h264_keyframe.extend_from_slice(&[0, 0, 0, 1, 0x65]); // IDR (type 5)
    h264_keyframe.extend_from_slice(&vec![0xAA; 100]); // 模拟数据

    let keyframe_info = orchestrator
        .process_keyframe(&stream_id, &h264_keyframe, Utc::now())
        .await?;

    if let Some(info) = keyframe_info {
        println!("✓ 关键帧已提取");
        println!("  文件路径: {}", info.file_path);
        println!("  大小: {} bytes", info.size);
    }

    // 7. 获取 Snapshot (Keyframe 模式)
    println!("\n--- 获取 Snapshot (Keyframe 模式) ---");
    let req = SnapshotRequest {
        stream_id: stream_id.clone(),
        mode: SnapshotMode::Keyframe,
        width: None,
        height: None,
    };

    let snapshot = orchestrator.get_snapshot(req).await?;
    println!("✓ Snapshot 获取成功");
    println!("  模式: {:?}", snapshot.mode_used);
    println!("  大小: {} bytes", snapshot.data.len());
    println!("  时间戳: {}", snapshot.timestamp);

    // 8. 获取 Snapshot (Auto 模式)
    println!("\n--- 获取 Snapshot (Auto 模式) ---");
    let req = SnapshotRequest {
        stream_id: stream_id.clone(),
        mode: SnapshotMode::Auto,
        width: Some(640),
        height: Some(480),
    };

    let snapshot = orchestrator.get_snapshot(req).await?;
    println!("✓ Snapshot 获取成功");
    println!("  模式: {:?}", snapshot.mode_used);
    println!("  大小: {} bytes", snapshot.data.len());

    // 9. 列出存储的对象
    println!("\n--- 列出存储对象 ---");
    let start = Utc::now() - chrono::Duration::hours(1);
    let end = Utc::now() + chrono::Duration::hours(1);
    let objects = storage.list_objects(&stream_id, start, end).await?;

    println!("找到 {} 个对象:", objects.len());
    for obj in objects {
        println!("  - {} ({} bytes)", obj.timestamp, obj.size);
    }

    println!("\n=== 示例完成 ===");
    Ok(())
}
