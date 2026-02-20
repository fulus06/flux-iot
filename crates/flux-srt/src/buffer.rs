use bytes::Bytes;
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

/// 发送缓冲区项
#[derive(Debug, Clone)]
pub struct SendBufferItem {
    pub sequence: u32,
    pub data: Bytes,
    pub timestamp: Instant,
    pub retransmit_count: u32,
}

/// 发送缓冲区（用于重传）
pub struct SendBuffer {
    buffer: BTreeMap<u32, SendBufferItem>,
    max_size: usize,
}

impl SendBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: BTreeMap::new(),
            max_size,
        }
    }

    /// 添加数据包到缓冲区
    pub fn insert(&mut self, sequence: u32, data: Bytes) -> bool {
        if self.buffer.len() >= self.max_size {
            return false;
        }

        self.buffer.insert(
            sequence,
            SendBufferItem {
                sequence,
                data,
                timestamp: Instant::now(),
                retransmit_count: 0,
            },
        );
        true
    }

    /// 获取数据包（用于重传）
    pub fn get(&self, sequence: u32) -> Option<&SendBufferItem> {
        self.buffer.get(&sequence)
    }

    /// 标记数据包已确认（移除）
    pub fn ack(&mut self, sequence: u32) {
        self.buffer.remove(&sequence);
    }

    /// 批量确认（移除所有 <= sequence 的包）
    pub fn ack_range(&mut self, up_to_sequence: u32) {
        self.buffer.retain(|&seq, _| seq > up_to_sequence);
    }

    /// 获取需要重传的包（超时）
    pub fn get_timeout_packets(&mut self, timeout_ms: u64) -> Vec<u32> {
        let now = Instant::now();
        let mut timeout_seqs = Vec::new();

        for (seq, item) in &mut self.buffer {
            if now.duration_since(item.timestamp).as_millis() > timeout_ms as u128 {
                timeout_seqs.push(*seq);
                item.retransmit_count += 1;
                item.timestamp = now; // 更新时间戳
            }
        }

        timeout_seqs
    }

    /// 获取缓冲区大小
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// 接收缓冲区项
#[derive(Debug, Clone)]
pub struct ReceiveBufferItem {
    pub sequence: u32,
    pub data: Bytes,
    pub timestamp: Instant,
}

/// 接收缓冲区（用于乱序包重组）
pub struct ReceiveBuffer {
    buffer: BTreeMap<u32, ReceiveBufferItem>,
    next_expected_seq: u32,
    max_size: usize,
}

impl ReceiveBuffer {
    pub fn new(initial_seq: u32, max_size: usize) -> Self {
        Self {
            buffer: BTreeMap::new(),
            next_expected_seq: initial_seq,
            max_size,
        }
    }

    /// 插入数据包
    pub fn insert(&mut self, sequence: u32, data: Bytes) -> bool {
        if self.buffer.len() >= self.max_size {
            return false;
        }

        // 忽略已接收的包
        if sequence < self.next_expected_seq {
            return true;
        }

        self.buffer.insert(
            sequence,
            ReceiveBufferItem {
                sequence,
                data,
                timestamp: Instant::now(),
            },
        );
        true
    }

    /// 提取连续的数据包
    pub fn pop_continuous(&mut self) -> Vec<Bytes> {
        let mut result = Vec::new();

        while let Some(item) = self.buffer.remove(&self.next_expected_seq) {
            result.push(item.data);
            self.next_expected_seq = self.next_expected_seq.wrapping_add(1);
        }

        result
    }

    /// 检测丢失的包序列号
    pub fn detect_missing(&self) -> Vec<u32> {
        let mut missing = Vec::new();

        if self.buffer.is_empty() {
            return missing;
        }

        let max_seq = *self.buffer.keys().max().unwrap();

        for seq in self.next_expected_seq..max_seq {
            if !self.buffer.contains_key(&seq) {
                missing.push(seq);
            }
        }

        missing
    }

    /// 获取下一个期望的序列号
    pub fn next_expected_seq(&self) -> u32 {
        self.next_expected_seq
    }

    /// 获取缓冲区大小
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_buffer_insert_and_ack() {
        let mut buffer = SendBuffer::new(10);

        assert!(buffer.insert(1, Bytes::from("data1")));
        assert!(buffer.insert(2, Bytes::from("data2")));
        assert_eq!(buffer.len(), 2);

        buffer.ack(1);
        assert_eq!(buffer.len(), 1);
        assert!(buffer.get(1).is_none());
        assert!(buffer.get(2).is_some());
    }

    #[test]
    fn test_send_buffer_ack_range() {
        let mut buffer = SendBuffer::new(10);

        buffer.insert(1, Bytes::from("data1"));
        buffer.insert(2, Bytes::from("data2"));
        buffer.insert(3, Bytes::from("data3"));
        buffer.insert(5, Bytes::from("data5"));

        buffer.ack_range(3);
        assert_eq!(buffer.len(), 1);
        assert!(buffer.get(5).is_some());
    }

    #[test]
    fn test_receive_buffer_continuous() {
        let mut buffer = ReceiveBuffer::new(1, 10);

        buffer.insert(1, Bytes::from("data1"));
        buffer.insert(2, Bytes::from("data2"));
        buffer.insert(3, Bytes::from("data3"));

        let continuous = buffer.pop_continuous();
        assert_eq!(continuous.len(), 3);
        assert_eq!(buffer.next_expected_seq(), 4);
    }

    #[test]
    fn test_receive_buffer_out_of_order() {
        let mut buffer = ReceiveBuffer::new(1, 10);

        buffer.insert(3, Bytes::from("data3"));
        buffer.insert(1, Bytes::from("data1"));
        buffer.insert(2, Bytes::from("data2"));

        let continuous = buffer.pop_continuous();
        assert_eq!(continuous.len(), 3);
        assert_eq!(buffer.next_expected_seq(), 4);
    }

    #[test]
    fn test_receive_buffer_detect_missing() {
        let mut buffer = ReceiveBuffer::new(1, 10);

        buffer.insert(1, Bytes::from("data1"));
        buffer.insert(3, Bytes::from("data3"));
        buffer.insert(5, Bytes::from("data5"));

        buffer.pop_continuous(); // 提取 seq=1

        let missing = buffer.detect_missing();
        assert_eq!(missing, vec![2, 4]);
    }

    #[test]
    fn test_send_buffer_max_size() {
        let mut buffer = SendBuffer::new(2);

        assert!(buffer.insert(1, Bytes::from("data1")));
        assert!(buffer.insert(2, Bytes::from("data2")));
        assert!(!buffer.insert(3, Bytes::from("data3"))); // 超过最大大小

        assert_eq!(buffer.len(), 2);
    }
}
