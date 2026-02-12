#[cfg(test)]
mod tests {
    use super::super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    // 模拟 H.264 数据（带起始码的 SPS + PPS + IDR）
    fn create_test_h264_data() -> Vec<u8> {
        let mut data = Vec::new();
        
        // SPS (NALU type 7)
        data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
        data.push(0x67); // NALU header (type 7 = SPS)
        data.extend_from_slice(&[0x42, 0x00, 0x1f, 0xe9]); // 模拟 SPS 数据
        
        // PPS (NALU type 8)
        data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
        data.push(0x68); // NALU header (type 8 = PPS)
        data.extend_from_slice(&[0xce, 0x3c, 0x80]); // 模拟 PPS 数据
        
        // IDR (NALU type 5)
        data.extend_from_slice(&[0, 0, 0, 1]); // 起始码
        data.push(0x65); // NALU header (type 5 = IDR)
        data.extend_from_slice(&[0x88, 0x84, 0x00, 0x10]); // 模拟 IDR 数据
        
        data
    }

    #[tokio::test]
    async fn test_extract_keyframe() {
        let temp_dir = TempDir::new().unwrap();
        let mut extractor = KeyframeExtractor::new(temp_dir.path().to_path_buf());

        let stream_id = "test_stream";
        let data = create_test_h264_data();
        let timestamp = Utc::now();

        // 第一次提取应该成功
        let result = extractor.process(stream_id, &data, timestamp).await.unwrap();
        assert!(result.is_some());

        let keyframe = result.unwrap();
        assert_eq!(keyframe.stream_id, stream_id);
        assert!(keyframe.data.len() > 0);
        assert!(std::path::Path::new(&keyframe.file_path).exists());
    }

    #[tokio::test]
    async fn test_extract_interval() {
        let temp_dir = TempDir::new().unwrap();
        let mut extractor = KeyframeExtractor::new(temp_dir.path().to_path_buf())
            .with_interval(5); // 5 秒间隔

        let stream_id = "test_stream";
        let data = create_test_h264_data();
        let timestamp1 = Utc::now();

        // 第一次提取
        let result1 = extractor.process(stream_id, &data, timestamp1).await.unwrap();
        assert!(result1.is_some());

        // 立即再次提取（应该被跳过）
        let timestamp2 = timestamp1 + chrono::Duration::seconds(2);
        let result2 = extractor.process(stream_id, &data, timestamp2).await.unwrap();
        assert!(result2.is_none());

        // 6 秒后提取（应该成功）
        let timestamp3 = timestamp1 + chrono::Duration::seconds(6);
        let result3 = extractor.process(stream_id, &data, timestamp3).await.unwrap();
        assert!(result3.is_some());
    }

    #[test]
    fn test_h264_parser() {
        use crate::codec::{H264Parser, H264NaluType};

        let mut parser = H264Parser::new();
        let data = create_test_h264_data();

        let nalus = parser.parse_annexb(&data);

        // 应该解析出 3 个 NALU
        assert_eq!(nalus.len(), 3);

        // 验证类型
        assert_eq!(nalus[0].nalu_type, H264NaluType::Sps);
        assert_eq!(nalus[1].nalu_type, H264NaluType::Pps);
        assert_eq!(nalus[2].nalu_type, H264NaluType::Idr);

        // 验证 IDR 是关键帧
        assert!(nalus[2].is_keyframe());

        // 验证参数集已缓存
        assert!(parser.has_parameter_sets());
        assert!(parser.sps().is_some());
        assert!(parser.pps().is_some());
    }

    #[test]
    fn test_nalu_type_detection() {
        use crate::codec::H264NaluType;

        let idr = H264NaluType::Idr;
        assert!(idr.is_keyframe());
        assert!(!idr.is_parameter_set());

        let sps = H264NaluType::Sps;
        assert!(!sps.is_keyframe());
        assert!(sps.is_parameter_set());

        let pps = H264NaluType::Pps;
        assert!(!pps.is_keyframe());
        assert!(pps.is_parameter_set());
    }
}
