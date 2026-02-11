use tokio::sync::broadcast;
use flux_types::message::Message;
use std::sync::Arc;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Message>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.sender.subscribe()
    }

    pub fn publish(&self, message: Message) -> Result<usize, broadcast::error::SendError<Message>> {
        self.sender.send(message)
    }
}

pub type SharedEventBus = Arc<EventBus>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_eventbus_publish_subscribe() {
        let bus = EventBus::new(10);
        let mut rx = bus.subscribe();

        let msg = Message::new("test/topic".to_string(), json!({"value": 42}));
        
        // 发布消息
        let result = bus.publish(msg.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // 1 个订阅者

        // 接收消息
        let received = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("Timeout waiting for message")
            .expect("Failed to receive message");
        
        assert_eq!(received.topic, "test/topic");
        assert_eq!(received.payload["value"], 42);
    }

    #[tokio::test]
    async fn test_eventbus_multiple_subscribers() {
        let bus = EventBus::new(10);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        let mut rx3 = bus.subscribe();

        let msg = Message::new("broadcast".to_string(), json!({"data": "hello"}));
        
        // 发布消息
        let result = bus.publish(msg.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3); // 3 个订阅者

        // 所有订阅者都应该收到消息
        let msg1 = rx1.recv().await.expect("rx1 failed");
        let msg2 = rx2.recv().await.expect("rx2 failed");
        let msg3 = rx3.recv().await.expect("rx3 failed");

        assert_eq!(msg1.topic, "broadcast");
        assert_eq!(msg2.topic, "broadcast");
        assert_eq!(msg3.topic, "broadcast");
    }

    #[tokio::test]
    async fn test_eventbus_no_subscribers() {
        let bus = EventBus::new(10);
        
        // 先订阅再取消订阅
        let _rx = bus.subscribe();
        drop(_rx);
        
        let msg = Message::new("empty".to_string(), json!({}));
        
        // 没有活跃订阅者时，broadcast 会返回错误
        let result = bus.publish(msg);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_eventbus_subscriber_drops() {
        let bus = EventBus::new(10);
        
        {
            let _rx = bus.subscribe();
            let msg = Message::new("test".to_string(), json!({}));
            assert_eq!(bus.publish(msg).unwrap(), 1);
        } // rx 被 drop
        
        // 订阅者被 drop 后，发布会失败（没有活跃订阅者）
        let msg = Message::new("test".to_string(), json!({}));
        assert!(bus.publish(msg).is_err());
    }

    #[tokio::test]
    async fn test_eventbus_capacity_overflow() {
        let bus = EventBus::new(2); // 容量为 2
        let mut rx = bus.subscribe();

        // 发布 3 条消息（超过容量）
        bus.publish(Message::new("msg1".to_string(), json!({"id": 1}))).unwrap();
        bus.publish(Message::new("msg2".to_string(), json!({"id": 2}))).unwrap();
        bus.publish(Message::new("msg3".to_string(), json!({"id": 3}))).unwrap();

        // 第一条消息被丢弃，接收时会得到 Lagged 错误
        match rx.recv().await {
            Err(broadcast::error::RecvError::Lagged(n)) => {
                assert_eq!(n, 1); // 丢失了 1 条消息
            },
            _ => panic!("Expected Lagged error"),
        }

        // 现在可以正常接收后续消息
        let msg2 = rx.recv().await.expect("Failed to receive msg2");
        assert_eq!(msg2.payload["id"], 2);

        let msg3 = rx.recv().await.expect("Failed to receive msg3");
        assert_eq!(msg3.payload["id"], 3);
    }

    #[tokio::test]
    async fn test_eventbus_clone() {
        let bus1 = EventBus::new(10);
        let bus2 = bus1.clone();

        let mut rx = bus1.subscribe();

        // 从 bus2 发布消息
        let msg = Message::new("clone_test".to_string(), json!({"source": "bus2"}));
        bus2.publish(msg).unwrap();

        // bus1 的订阅者应该能收到
        let received = rx.recv().await.expect("Failed to receive");
        assert_eq!(received.topic, "clone_test");
    }

    #[tokio::test]
    async fn test_eventbus_concurrent_publish() {
        let bus = Arc::new(EventBus::new(100));
        let mut rx = bus.subscribe();

        // 并发发布 10 条消息
        let mut handles = vec![];
        for i in 0..10 {
            let bus_clone = bus.clone();
            let handle = tokio::spawn(async move {
                let msg = Message::new(
                    format!("topic/{}", i),
                    json!({"index": i})
                );
                bus_clone.publish(msg).unwrap();
            });
            handles.push(handle);
        }

        // 等待所有发布完成
        for handle in handles {
            handle.await.unwrap();
        }

        // 应该收到 10 条消息
        let mut count = 0;
        while let Ok(result) = timeout(Duration::from_millis(100), rx.recv()).await {
            if result.is_ok() {
                count += 1;
            }
        }
        assert_eq!(count, 10);
    }
}
