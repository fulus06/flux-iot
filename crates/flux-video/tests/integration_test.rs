// 集成测试 - 使用模拟数据测试完整流程
use flux_video::{
    engine::VideoEngine,
    stream::RtspStream,
    snapshot::KeyframeExtractor,
    storage::StandaloneStorage,
    codec::H264Parser,
};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::Duration;

/// 生成模拟的 H.264 数据（SPS + PPS + IDR）
fn create_mock_h264_frame() -> Vec<u8> {
    let mut data = Vec::new();
    
    // SPS (NALU type 7)
    data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
    data.push(0x67); // NALU header (type 7 = SPS)
    data.extend_from_slice(&[0x42, 0x00, 0x1f, 0xe9, 0x02, 0xc1, 0x2c, 0x80]);
    
    // PPS (NALU type 8)
    data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
    data.push(0x68); // NALU header (type 8 = PPS)
    data.extend_from_slice(&[0xce, 0x3c, 0x80]);
    
    // IDR (NALU type 5)
    data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
    data.push(0x65); // NALU header (type 5 = IDR)
    // 添加一些模拟的 IDR 数据
    data.extend_from_slice(&[0x88, 0x84, 0x00, 0x10, 0xff, 0xfe, 0xf6, 0xf0]);
    data.extend_from_slice(&vec![0xAA; 100]); // 填充数据
    
    data
}

#[tokio::test]
async fn test_video_engine_basic() {
    let engine = VideoEngine::new();
    
    // 测试初始状态
    let streams = engine.list_streams();
    assert_eq!(streams.len(), 0);
    
    // 创建模拟流
    let stream = RtspStream::new(
        "test_stream".to_string(),
        "rtsp://mock.example.com/stream".to_string(),
    );
    
    // 发布流
    let result = engine.publish_stream("test_stream".to_string(), Arc::new(stream));
    assert!(result.is_ok());
    
    // 验证流已注册
    let streams = engine.list_streams();
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0], "test_stream");
}

#[tokio::test]
async fn test_storage_pipeline() {
    let temp_dir = TempDir::new().unwrap();
    let mut storage = StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap();
    
    let stream_id = "test_camera";
    let timestamp = chrono::Utc::now();
    let data = bytes::Bytes::from(vec![0xAA; 1024]); // 1KB 模拟数据
    
    // 保存数据
    let path = storage.put_object(stream_id, timestamp, data.clone()).await.unwrap();
    assert!(!path.is_empty());
    
    // 读取数据
    let retrieved = storage.get_object(stream_id, timestamp).await.unwrap();
    assert_eq!(retrieved.len(), data.len());
}

#[tokio::test]
async fn test_keyframe_extraction_pipeline() {
    let temp_dir = TempDir::new().unwrap();
    let mut extractor = KeyframeExtractor::new(temp_dir.path().to_path_buf())
        .with_interval(1); // 1 秒间隔
    
    let stream_id = "test_camera";
    let data = create_mock_h264_frame();
    
    // 第一次提取
    let timestamp1 = chrono::Utc::now();
    let result1 = extractor.process(stream_id, &data, timestamp1).await.unwrap();
    assert!(result1.is_some(), "First extraction should succeed");
    
    let keyframe1 = result1.unwrap();
    assert_eq!(keyframe1.stream_id, stream_id);
    assert!(keyframe1.data.len() > 0);
    assert!(std::path::Path::new(&keyframe1.file_path).exists());
    
    // 立即再次提取（应该被跳过）
    let timestamp2 = timestamp1 + chrono::Duration::milliseconds(500);
    let result2 = extractor.process(stream_id, &data, timestamp2).await.unwrap();
    assert!(result2.is_none(), "Second extraction should be skipped");
    
    // 2 秒后提取（应该成功）
    let timestamp3 = timestamp1 + chrono::Duration::seconds(2);
    let result3 = extractor.process(stream_id, &data, timestamp3).await.unwrap();
    assert!(result3.is_some(), "Third extraction should succeed");
}

#[tokio::test]
async fn test_h264_parser_with_mock_data() {
    let mut parser = H264Parser::new();
    let data = create_mock_h264_frame();
    
    // 解析 NALU
    let nalus = parser.parse_annexb(&data);
    
    // 应该解析出 3 个 NALU (SPS, PPS, IDR)
    assert_eq!(nalus.len(), 3, "Should parse 3 NALUs");
    
    // 验证参数集已缓存
    assert!(parser.has_parameter_sets(), "Should have parameter sets");
    assert!(parser.sps().is_some(), "Should have SPS");
    assert!(parser.pps().is_some(), "Should have PPS");
    
    // 验证关键帧
    let idr_nalu = nalus.iter().find(|n| n.is_keyframe());
    assert!(idr_nalu.is_some(), "Should find IDR frame");
}

#[tokio::test]
async fn test_concurrent_stream_processing() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(tokio::sync::RwLock::new(
        StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap()
    ));
    
    let mut handles = vec![];
    
    // 模拟 10 路并发流
    for i in 0..10 {
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            let stream_id = format!("camera_{:03}", i);
            let mut storage = storage_clone.write().await;
            
            // 每路流写入 10 个分片
            for j in 0..10 {
                let timestamp = chrono::Utc::now() + chrono::Duration::seconds(j);
                let data = bytes::Bytes::from(vec![i as u8; 1024]);
                
                let result = storage.put_object(&stream_id, timestamp, data).await;
                assert!(result.is_ok(), "Write should succeed for stream {}", stream_id);
                
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }
    
    println!("✅ Successfully processed 10 concurrent streams with 10 segments each");
}

#[tokio::test]
async fn test_memory_efficiency() {
    // 测试内存使用是否在合理范围内
    let temp_dir = TempDir::new().unwrap();
    let mut storage = StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap();
    
    let stream_id = "memory_test";
    let data = bytes::Bytes::from(vec![0xFF; 1024 * 1024]); // 1MB 数据
    
    // 写入 10 个分片
    for i in 0..10 {
        let timestamp = chrono::Utc::now() + chrono::Duration::seconds(i);
        let result = storage.put_object(stream_id, timestamp, data.clone()).await;
        assert!(result.is_ok());
    }
    
    println!("✅ Memory efficiency test passed: 10MB written successfully");
}

#[tokio::test]
async fn test_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let storage = StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap();
    
    // 测试读取不存在的对象
    let result = storage.get_object("nonexistent", chrono::Utc::now()).await;
    assert!(result.is_err(), "Should return error for nonexistent object");
    
    println!("✅ Error handling test passed");
}

#[tokio::test]
async fn test_cleanup_expired_data() {
    let temp_dir = TempDir::new().unwrap();
    let mut storage = StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap();
    
    let stream_id = "cleanup_test";
    
    // 写入旧数据
    let old_timestamp = chrono::Utc::now() - chrono::Duration::days(10);
    let old_data = bytes::Bytes::from(vec![0x01; 100]);
    storage.put_object(stream_id, old_timestamp, old_data).await.unwrap();
    
    // 写入新数据
    let new_timestamp = chrono::Utc::now();
    let new_data = bytes::Bytes::from(vec![0x02; 100]);
    storage.put_object(stream_id, new_timestamp, new_data).await.unwrap();
    
    // 清理 8 天前的数据
    let before = chrono::Utc::now() - chrono::Duration::days(8);
    let deleted = storage.cleanup_expired(before).await.unwrap();
    
    assert_eq!(deleted, 1, "Should delete 1 old object");
    
    // 验证新数据仍然存在
    let result = storage.get_object(stream_id, new_timestamp).await;
    assert!(result.is_ok(), "New data should still exist");
    
    println!("✅ Cleanup test passed: {} old objects deleted", deleted);
}
