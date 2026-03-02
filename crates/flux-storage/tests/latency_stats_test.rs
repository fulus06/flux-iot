use flux_storage::{LocalBackend, StorageBackend};
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_local_backend_latency_tracking() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let backend = LocalBackend::new(temp_dir.path().to_path_buf());

    // 初始状态：延迟应该为 0
    let stats = backend.stats().await;
    assert_eq!(stats.read_count, 0);
    assert_eq!(stats.write_count, 0);
    assert_eq!(stats.avg_read_latency_ms, 0.0);
    assert_eq!(stats.avg_write_latency_ms, 0.0);

    // 执行写入操作
    let test_data = b"Hello, World!";
    backend.write("test.txt", test_data).await.unwrap();

    // 检查写入统计
    let stats = backend.stats().await;
    assert_eq!(stats.write_count, 1);
    assert_eq!(stats.bytes_written, test_data.len() as u64);
    assert!(stats.avg_write_latency_ms >= 0.0, "Write latency should be non-negative");
    println!("Write latency: {:.3} ms", stats.avg_write_latency_ms);

    // 执行读取操作
    let data = backend.read("test.txt").await.unwrap();
    assert_eq!(data.as_ref(), test_data);

    // 检查读取统计
    let stats = backend.stats().await;
    assert_eq!(stats.read_count, 1);
    assert_eq!(stats.bytes_read, test_data.len() as u64);
    assert!(stats.avg_read_latency_ms >= 0.0, "Read latency should be non-negative");
    println!("Read latency: {:.3} ms", stats.avg_read_latency_ms);

    // 执行多次操作以测试平均值计算
    for i in 0..10 {
        let path = format!("test_{}.txt", i);
        backend.write(&path, test_data).await.unwrap();
        backend.read(&path).await.unwrap();
    }

    // 检查平均延迟
    let stats = backend.stats().await;
    assert_eq!(stats.write_count, 11); // 1 + 10
    assert_eq!(stats.read_count, 11);  // 1 + 10
    assert!(stats.avg_write_latency_ms >= 0.0);
    assert!(stats.avg_read_latency_ms >= 0.0);
    
    println!("After 11 operations:");
    println!("  Average write latency: {:.3} ms", stats.avg_write_latency_ms);
    println!("  Average read latency: {:.3} ms", stats.avg_read_latency_ms);
    println!("  Total bytes written: {}", stats.bytes_written);
    println!("  Total bytes read: {}", stats.bytes_read);
}

#[tokio::test]
async fn test_latency_calculation_accuracy() {
    let temp_dir = TempDir::new().unwrap();
    let backend = LocalBackend::new(temp_dir.path().to_path_buf());

    // 写入不同大小的文件
    let sizes = vec![100, 1000, 10000, 100000];
    
    for (i, size) in sizes.iter().enumerate() {
        let data = vec![0u8; *size];
        let path = format!("file_{}.dat", i);
        backend.write(&path, &data).await.unwrap();
    }

    let stats = backend.stats().await;
    assert_eq!(stats.write_count, sizes.len() as u64);
    
    // 延迟应该是合理的（通常小于 100ms）
    assert!(stats.avg_write_latency_ms < 100.0, 
        "Average write latency too high: {:.3} ms", stats.avg_write_latency_ms);
    
    println!("Write latency for different file sizes:");
    println!("  Files written: {}", stats.write_count);
    println!("  Average latency: {:.3} ms", stats.avg_write_latency_ms);
    println!("  Total bytes: {}", stats.bytes_written);
}

#[tokio::test]
async fn test_concurrent_operations_latency() {
    let temp_dir = TempDir::new().unwrap();
    let backend = LocalBackend::new(temp_dir.path().to_path_buf());

    // 并发写入
    let mut tasks = vec![];
    for i in 0..20 {
        let backend_clone = backend.clone();
        let task = tokio::spawn(async move {
            let data = format!("Data {}", i);
            let path = format!("concurrent_{}.txt", i);
            backend_clone.write(&path, data.as_bytes()).await
        });
        tasks.push(task);
    }

    // 等待所有任务完成
    for task in tasks {
        task.await.unwrap().unwrap();
    }

    let stats = backend.stats().await;
    assert_eq!(stats.write_count, 20);
    assert!(stats.avg_write_latency_ms >= 0.0);
    
    println!("Concurrent operations:");
    println!("  Operations: {}", stats.write_count);
    println!("  Average latency: {:.3} ms", stats.avg_write_latency_ms);
}
