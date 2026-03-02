use bytes::Bytes;
use flux_rtspd::h264_depacketizer::H264Depacketizer;
use flux_rtspd::h265_depacketizer::H265Depacketizer;
use flux_rtspd::rtp_receiver::RtpPacket;

#[test]
fn test_h264_fu_b_fragmentation() {
    let mut depacketizer = H264Depacketizer::new();
    
    // FU-B 开始包 (Type 29, with DON)
    let payload1 = Bytes::from(vec![
        0x7D, // FU indicator (Type 29 = FU-B)
        0x85, // FU header: S=1, E=0, Type=5 (IDR)
        0x00, 0x01, // DON = 1
        0x01, 0x02, 0x03, // Payload
    ]);
    let packet1 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: false,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 3000,
        ssrc: 0x12345678,
        payload: payload1,
    };
    
    let nalus1 = depacketizer.process_rtp(packet1).unwrap();
    assert_eq!(nalus1.len(), 0); // 未完成
    
    // FU-B 中间包
    let payload2 = Bytes::from(vec![
        0x7D, // FU indicator
        0x05, // FU header: S=0, E=0, Type=5
        0x00, 0x02, // DON = 2
        0x04, 0x05, 0x06,
    ]);
    let packet2 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: false,
        payload_type: 96,
        sequence_number: 2,
        timestamp: 3000,
        ssrc: 0x12345678,
        payload: payload2,
    };
    
    let nalus2 = depacketizer.process_rtp(packet2).unwrap();
    assert_eq!(nalus2.len(), 0); // 仍未完成
    
    // FU-B 结束包
    let payload3 = Bytes::from(vec![
        0x7D, // FU indicator
        0x45, // FU header: S=0, E=1, Type=5
        0x00, 0x03, // DON = 3
        0x07, 0x08, 0x09,
    ]);
    let packet3 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 3,
        timestamp: 3000,
        ssrc: 0x12345678,
        payload: payload3,
    };
    
    let nalus3 = depacketizer.process_rtp(packet3).unwrap();
    assert_eq!(nalus3.len(), 1);
    assert!(nalus3[0].is_keyframe);
    assert_eq!(nalus3[0].timestamp, 3000);
    
    // 验证组装后的数据
    let assembled_data = &nalus3[0].data;
    assert_eq!(assembled_data[0] & 0x1F, 5); // NAL type should be 5 (IDR)
}

#[test]
fn test_h265_paci_single_nalu() {
    let mut depacketizer = H265Depacketizer::new();
    
    // PACI 包 (Type 50) - Single NAL unit mode
    let payload = Bytes::from(vec![
        0x64, 0x01, // Payload header: Type=50 (PACI)
        0x00,       // PACI header: A=0 (single), PHSsize=0
        // NAL unit data (Type 19 = IDR)
        0x26, 0x01, 0x01, 0x02, 0x03,
    ]);
    let packet = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 4000,
        ssrc: 0x12345678,
        payload,
    };
    
    let nalus = depacketizer.process_rtp(packet).unwrap();
    assert_eq!(nalus.len(), 1);
    assert!(nalus[0].is_keyframe);
    assert_eq!(nalus[0].timestamp, 4000);
}

#[test]
fn test_h265_paci_aggregation() {
    let mut depacketizer = H265Depacketizer::new();
    
    // PACI 包 (Type 50) - Aggregation mode
    let payload = Bytes::from(vec![
        0x64, 0x01, // Payload header: Type=50 (PACI)
        0x80,       // PACI header: A=1 (aggregation), PHSsize=0
        // First NAL unit
        0x00, 0x05, // Size = 5
        0x26, 0x01, 0x01, 0x02, 0x03,
        // Second NAL unit
        0x00, 0x04, // Size = 4
        0x40, 0x01, 0x04, 0x05,
    ]);
    let packet = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 5000,
        ssrc: 0x12345678,
        payload,
    };
    
    let nalus = depacketizer.process_rtp(packet).unwrap();
    assert_eq!(nalus.len(), 2);
    assert!(nalus[0].is_keyframe); // Type 19 (IDR)
    assert!(nalus[1].is_keyframe); // Type 32 (VPS) is also considered keyframe
}

#[test]
fn test_h264_fu_b_timestamp_mismatch() {
    let mut depacketizer = H264Depacketizer::new();
    
    // FU-B 开始包
    let payload1 = Bytes::from(vec![
        0x7D, 0x85, 0x00, 0x01, 0x01, 0x02,
    ]);
    let packet1 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: false,
        payload_type: 96,
        sequence_number: 1,
        timestamp: 1000,
        ssrc: 0x12345678,
        payload: payload1,
    };
    
    depacketizer.process_rtp(packet1).unwrap();
    
    // FU-B 结束包，但时间戳不匹配
    let payload2 = Bytes::from(vec![
        0x7D, 0x45, 0x00, 0x02, 0x03, 0x04,
    ]);
    let packet2 = RtpPacket {
        version: 2,
        padding: false,
        extension: false,
        csrc_count: 0,
        marker: true,
        payload_type: 96,
        sequence_number: 2,
        timestamp: 2000, // 不同的时间戳
        ssrc: 0x12345678,
        payload: payload2,
    };
    
    let nalus = depacketizer.process_rtp(packet2).unwrap();
    assert_eq!(nalus.len(), 0); // 应该被丢弃
}
