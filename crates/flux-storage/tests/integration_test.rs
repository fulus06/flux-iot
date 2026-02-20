use flux_storage::{
    LocalBackend, PoolConfig, SegmentStorage, SegmentStorageImpl, StorageBackend, StorageManager,
};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_storage_manager_with_backends() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalBackend::new(temp_dir.path().to_path_buf()));
    
    let config = PoolConfig {
        name: "test-pool".to_string(),
        path: temp_dir.path().to_path_buf(),
        disk_type: flux_storage::DiskType::SSD,
        priority: 1,
        max_usage_percent: 90.0,
    };
    
    let manager = Arc::new(StorageManager::new());
    manager
        .initialize_with_backends(vec![(config, backend)])
        .await
        .unwrap();
    
    // 测试写入
    manager
        .write_to_pool("test-pool", "test.txt", b"Hello, World!")
        .await
        .unwrap();
    
    // 测试读取
    let data = manager
        .read_from_pool("test-pool", "test.txt")
        .await
        .unwrap();
    assert_eq!(&data[..], b"Hello, World!");
    
    // 测试删除
    manager
        .delete_from_pool("test-pool", "test.txt")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_storage_manager_write_with_selection() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalBackend::new(temp_dir.path().to_path_buf()));
    
    let config = PoolConfig {
        name: "auto-pool".to_string(),
        path: temp_dir.path().to_path_buf(),
        disk_type: flux_storage::DiskType::SSD,
        priority: 1,
        max_usage_percent: 90.0,
    };
    
    let manager = Arc::new(StorageManager::new());
    manager
        .initialize_with_backends(vec![(config, backend)])
        .await
        .unwrap();
    
    // 测试自动选择池并写入
    let pool_name = manager
        .write_with_selection("auto/test.txt", b"Auto selected!")
        .await
        .unwrap();
    
    assert_eq!(pool_name, "auto-pool");
    
    // 验证可以读取
    let data = manager
        .read_from_any_pool("auto/test.txt")
        .await
        .unwrap();
    assert_eq!(&data[..], b"Auto selected!");
}

#[tokio::test]
async fn test_storage_manager_list_from_all_pools() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalBackend::new(temp_dir.path().to_path_buf()));
    
    let config = PoolConfig {
        name: "list-pool".to_string(),
        path: temp_dir.path().to_path_buf(),
        disk_type: flux_storage::DiskType::SSD,
        priority: 1,
        max_usage_percent: 90.0,
    };
    
    let manager = Arc::new(StorageManager::new());
    manager
        .initialize_with_backends(vec![(config, backend)])
        .await
        .unwrap();
    
    // 写入多个文件
    manager
        .write_to_pool("list-pool", "dir/file1.txt", b"data1")
        .await
        .unwrap();
    manager
        .write_to_pool("list-pool", "dir/file2.txt", b"data2")
        .await
        .unwrap();
    
    // 列出文件
    let files = manager.list_from_all_pools("dir").await.unwrap();
    assert_eq!(files.len(), 2);
}

#[tokio::test]
async fn test_segment_storage_impl() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalBackend::new(temp_dir.path().to_path_buf()));
    
    let config = PoolConfig {
        name: "segment-pool".to_string(),
        path: temp_dir.path().to_path_buf(),
        disk_type: flux_storage::DiskType::SSD,
        priority: 1,
        max_usage_percent: 90.0,
    };
    
    let manager = Arc::new(StorageManager::new());
    manager
        .initialize_with_backends(vec![(config, backend)])
        .await
        .unwrap();
    
    let segment_storage = SegmentStorageImpl::new(manager);
    
    // 测试保存分片
    let filename = segment_storage
        .save_segment("test-stream", 1, b"segment data")
        .await
        .unwrap();
    assert_eq!(filename, "segment_1.ts");
    
    // 测试加载分片
    let data = segment_storage
        .load_segment("test-stream", 1)
        .await
        .unwrap();
    assert_eq!(&data[..], b"segment data");
    
    // 测试列出分片
    segment_storage
        .save_segment("test-stream", 2, b"segment data 2")
        .await
        .unwrap();
    segment_storage
        .save_segment("test-stream", 3, b"segment data 3")
        .await
        .unwrap();
    
    let segments = segment_storage.list_segments("test-stream").await.unwrap();
    assert_eq!(segments, vec![1, 2, 3]);
    
    // 测试清理旧分片
    let deleted = segment_storage
        .cleanup_old_segments("test-stream", 2)
        .await
        .unwrap();
    assert_eq!(deleted, 1);
    
    let segments = segment_storage.list_segments("test-stream").await.unwrap();
    assert_eq!(segments, vec![2, 3]);
    
    // 测试删除分片
    segment_storage
        .delete_segment("test-stream", 2)
        .await
        .unwrap();
    
    let segments = segment_storage.list_segments("test-stream").await.unwrap();
    assert_eq!(segments, vec![3]);
}

#[tokio::test]
async fn test_segment_storage_impl_multiple_streams() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalBackend::new(temp_dir.path().to_path_buf()));
    
    let config = PoolConfig {
        name: "multi-stream-pool".to_string(),
        path: temp_dir.path().to_path_buf(),
        disk_type: flux_storage::DiskType::SSD,
        priority: 1,
        max_usage_percent: 90.0,
    };
    
    let manager = Arc::new(StorageManager::new());
    manager
        .initialize_with_backends(vec![(config, backend)])
        .await
        .unwrap();
    
    let segment_storage = SegmentStorageImpl::new(manager);
    
    // 保存多个流的分片
    segment_storage
        .save_segment("stream1", 1, b"stream1 data")
        .await
        .unwrap();
    segment_storage
        .save_segment("stream2", 1, b"stream2 data")
        .await
        .unwrap();
    
    // 验证每个流的分片独立
    let stream1_segments = segment_storage.list_segments("stream1").await.unwrap();
    let stream2_segments = segment_storage.list_segments("stream2").await.unwrap();
    
    assert_eq!(stream1_segments, vec![1]);
    assert_eq!(stream2_segments, vec![1]);
    
    // 验证数据正确
    let data1 = segment_storage.load_segment("stream1", 1).await.unwrap();
    let data2 = segment_storage.load_segment("stream2", 1).await.unwrap();
    
    assert_eq!(&data1[..], b"stream1 data");
    assert_eq!(&data2[..], b"stream2 data");
}
