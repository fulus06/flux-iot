#[cfg(test)]
mod tests {
    use super::super::standalone::*;
    use bytes::Bytes;
    use chrono::Utc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_put_and_get_object() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap();

        let stream_id = "test_stream";
        let timestamp = Utc::now();
        let data = Bytes::from("test video data");

        // 保存对象
        let path = storage.put_object(stream_id, timestamp, data.clone()).await.unwrap();
        assert!(!path.is_empty());

        // 读取对象
        let retrieved = storage.get_object(stream_id, timestamp).await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_list_objects() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap();

        let stream_id = "test_stream";
        let now = Utc::now();

        // 保存多个对象
        for i in 0..5 {
            let timestamp = now + chrono::Duration::seconds(i);
            let data = Bytes::from(format!("data_{}", i));
            storage.put_object(stream_id, timestamp, data).await.unwrap();
        }

        // 列出对象
        let start = now - chrono::Duration::seconds(1);
        let end = now + chrono::Duration::seconds(10);
        let objects = storage.list_objects(stream_id, start, end).await.unwrap();

        assert_eq!(objects.len(), 5);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = StandaloneStorage::new(temp_dir.path().to_path_buf()).unwrap();

        let stream_id = "test_stream";
        let old_timestamp = Utc::now() - chrono::Duration::days(10);
        let new_timestamp = Utc::now();

        // 保存旧对象
        storage.put_object(stream_id, old_timestamp, Bytes::from("old data")).await.unwrap();
        
        // 保存新对象
        storage.put_object(stream_id, new_timestamp, Bytes::from("new data")).await.unwrap();

        // 清理 8 天前的数据
        let before = Utc::now() - chrono::Duration::days(8);
        let deleted = storage.cleanup_expired(before).await.unwrap();

        assert_eq!(deleted, 1);

        // 验证新对象仍然存在
        let retrieved = storage.get_object(stream_id, new_timestamp).await;
        assert!(retrieved.is_ok());

        // 验证旧对象已删除
        let old_retrieved = storage.get_object(stream_id, old_timestamp).await;
        assert!(old_retrieved.is_err());
    }

    #[test]
    fn test_object_key_to_string() {
        let key = ObjectKey {
            stream_id: "cam001".to_string(),
            timestamp: chrono::DateTime::from_timestamp(1707624000, 0).unwrap(),
            object_type: ObjectType::VideoSegment,
        };

        let key_str = key.to_string();
        assert!(key_str.contains("cam001"));
        assert!(key_str.contains("1707624000"));
        assert!(key_str.contains("segment"));
    }
}
