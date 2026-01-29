use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

use ntex_mqtt::{v3, v5};
use tracing::{info, warn};

#[derive(Clone)]
pub enum MqttSink {
    V3(v3::MqttSink),
    V5(v5::MqttSink),
}

impl MqttSink {
    pub async fn publish(&self, topic: &str, payload: ntex::util::Bytes) -> bool {
        match self {
            MqttSink::V3(sink) => {
                 match sink.publish(ntex::util::ByteString::from(topic)).send_at_least_once(payload).await {
                     Ok(_) => true,
                     Err(e) => {
                         warn!("V3 publish failed: {:?}", e);
                         false
                     }
                 }
            }
            MqttSink::V5(sink) => {
                 match sink.publish(ntex::util::ByteString::from(topic)).send_at_least_once(payload).await {
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
    pub client_id: String,
    pub sink: MqttSink,
}

#[derive(Clone)]
pub struct MqttManager {
    sessions: Rc<RefCell<HashMap<String, SessionState>>>, 
}

impl MqttManager {
    pub fn new() -> Self {
        Self {
            sessions: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn add_v3(&self, client_id: String, sink: v3::MqttSink) {
        info!("Client connected (V3): {}", client_id);
        self.sessions.borrow_mut().insert(client_id.clone(), SessionState {
            client_id,
            sink: MqttSink::V3(sink),
        });
    }

    pub fn add_v5(&self, client_id: String, sink: v5::MqttSink) {
        info!("Client connected (V5): {}", client_id);
        self.sessions.borrow_mut().insert(client_id.clone(), SessionState {
             client_id,
             sink: MqttSink::V5(sink),
        });
    }

    pub fn remove(&self, client_id: &str) {
        info!("Client disconnected: {}", client_id);
        self.sessions.borrow_mut().remove(client_id);
    }
    
    pub async fn broadcast(&self, topic: &str, payload: ntex::util::Bytes) {
        let sinks: Vec<MqttSink> = self.sessions.borrow().values().map(|s| s.sink.clone()).collect();
        for sink in sinks {
            sink.publish(topic, payload.clone()).await;
        }
    }
}
