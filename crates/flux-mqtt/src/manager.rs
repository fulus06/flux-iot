use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ntex_mqtt::{v3, v5};
use tracing::{debug, info, warn};

use crate::acl::MqttAcl;
use crate::metrics::MqttMetrics;
use crate::retained::RetainedStore;
use crate::topic_matcher::TopicMatcher;

#[derive(Clone)]
pub enum MqttSink {
    V3(v3::MqttSink),
    V5(v5::MqttSink),
}

impl MqttSink {
    pub async fn publish(&self, topic: &str, payload: ntex::util::Bytes) -> bool {
        match self {
            MqttSink::V3(sink) => {
                match sink
                    .publish(ntex::util::ByteString::from(topic))
                    .send_at_least_once(payload)
                    .await
                {
                    Ok(_) => true,
                    Err(e) => {
                        warn!("V3 publish failed: {:?}", e);
                        false
                    }
                }
            }
            MqttSink::V5(sink) => {
                match sink
                    .publish(ntex::util::ByteString::from(topic))
                    .send_at_least_once(payload)
                    .await
                {
                    Ok(_) => true,
                    Err(e) => {
                        warn!("V5 publish failed: {:?}", e);
                        false
                    }
                }
            }
        }
    }
}

pub struct SessionState {
    #[allow(dead_code)]
    pub client_id: String,
    pub sink: MqttSink,
}

#[derive(Clone)]
pub struct MqttManager {
    sessions: Rc<RefCell<HashMap<String, SessionState>>>,
    retained: RetainedStore,
    topics: TopicMatcher,
    acl: Option<MqttAcl>,
    metrics: MqttMetrics,
}

impl Default for MqttManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MqttManager {
    pub fn new() -> Self {
        Self {
            sessions: Rc::new(RefCell::new(HashMap::new())),
            retained: RetainedStore::new(),
            topics: TopicMatcher::new(),
            acl: None,
            metrics: MqttMetrics::new(),
        }
    }

    /// 创建带 ACL 的管理器
    pub fn with_acl(mut self, acl: MqttAcl) -> Self {
        self.acl = Some(acl);
        self
    }

    /// 获取 ACL
    pub fn acl(&self) -> Option<&MqttAcl> {
        self.acl.as_ref()
    }

    /// 获取指标
    pub fn metrics(&self) -> &MqttMetrics {
        &self.metrics
    }

    pub fn add_v3(&self, client_id: String, sink: v3::MqttSink) {
        info!("Client connected (V3): {}", client_id);
        self.sessions.borrow_mut().insert(
            client_id.clone(),
            SessionState {
                client_id,
                sink: MqttSink::V3(sink),
            },
        );
        self.metrics.record_connection();
    }

    pub fn add_v5(&self, client_id: String, sink: v5::MqttSink) {
        info!("Client connected (V5): {}", client_id);
        self.sessions.borrow_mut().insert(
            client_id.clone(),
            SessionState {
                client_id,
                sink: MqttSink::V5(sink),
            },
        );
        self.metrics.record_connection();
    }

    pub fn remove(&self, client_id: &str) {
        info!("Client disconnected: {}", client_id);
        self.sessions.borrow_mut().remove(client_id);
        self.metrics.record_disconnection();
    }

    pub async fn broadcast(&self, topic: &str, payload: ntex::util::Bytes) {
        let sinks: Vec<MqttSink> = self
            .sessions
            .borrow()
            .values()
            .map(|s| s.sink.clone())
            .collect();
        for sink in sinks {
            sink.publish(topic, payload.clone()).await;
        }
    }

    /// 发布消息到匹配的订阅者
    pub async fn publish_to_subscribers(
        &self,
        topic: &str,
        payload: ntex::util::Bytes,
        retained: bool,
    ) {
        // 如果是 retained 消息，保存
        if retained {
            self.retained.set(topic.to_string(), payload.clone(), 1);
        }

        // 查找匹配的客户端
        let client_ids = self.topics.find_matching_clients(topic);

        debug!(
            topic = %topic,
            subscribers = client_ids.len(),
            retained = retained,
            "Publishing to subscribers"
        );

        // 发送给匹配的订阅者
        let sessions = self.sessions.borrow();
        for client_id in client_ids {
            if let Some(session) = sessions.get(&client_id) {
                session.sink.publish(topic, payload.clone()).await;
            }
        }
    }

    /// 订阅主题
    pub async fn subscribe(&self, client_id: &str, topic_filter: &str) {
        self.topics.subscribe(client_id.to_string(), topic_filter.to_string());
        self.metrics.record_subscription();

        // 发送匹配的 retained 消息
        let retained_msgs = self.retained.get_matching(topic_filter);
        if let Some(session) = self.sessions.borrow().get(client_id) {
            for msg in retained_msgs {
                session.sink.publish(&msg.topic, msg.payload).await;
                debug!(
                    client_id = %client_id,
                    topic = %msg.topic,
                    "Sent retained message"
                );
            }
        }
    }

    /// 取消订阅
    pub fn unsubscribe(&self, client_id: &str, topic_filter: &str) {
        self.topics.unsubscribe(client_id, topic_filter);
        self.metrics.record_unsubscription();
    }

    /// 获取 retained 消息存储
    pub fn retained_store(&self) -> &RetainedStore {
        &self.retained
    }

    /// 获取主题匹配器
    pub fn topic_matcher(&self) -> &TopicMatcher {
        &self.topics
    }
}
