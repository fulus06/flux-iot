use std::thread;
use std::sync::Arc;
use rumqttd::{Broker, Config, Notification};
use flux_types::message::Message;
use crate::AppState;

pub fn start_mqtt_broker(state: Arc<AppState>) {
    tracing::info!("Starting Embedded MQTT Broker on port 1883...");

    // Create a default config
    let config = config::create_default_config();

    // Triggering the broker in a separate thread
    // Broker::new returns (Broker, Link)
    // We move them into the thread
    
    thread::spawn(move || {
        let mut broker = Broker::new(config);
        let mut link = broker.link("flux-bridge").expect("Failed to create bridge link");
        
        // Spawn Bridge Task
        let state_clone = state.clone();
        thread::spawn(move || {
             // Link usage
             let (mut tx, mut rx) = link;
             tx.subscribe("#").expect("Failed to subscribe");
             
             loop {
                 // recv returns Result<Option<Notification>, Error>
                 match rx.recv() {
                     Ok(Some(notification)) => {
                         if let Notification::Forward(forward) = notification {
                              // Topic is Bytes
                              let topic_bytes = &forward.publish.topic;
                              let topic = std::str::from_utf8(topic_bytes)
                                  .unwrap_or("unknown/topic")
                                  .to_string();
                                  
                              let payload = forward.publish.payload.to_vec();
                              
                              // Simple JSON check
                              if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&payload) {
                                  let msg = Message::new(topic.clone(), json_val);
                                  if let Err(e) = state_clone.event_bus.publish(msg) {
                                      tracing::warn!("MQTT Bridge: EventBus publish error: {}", e);
                                  } else {
                                      tracing::debug!("Bridged via MQTT: {}", topic);
                                  }
                              }
                         }
                     }
                     Ok(None) => {
                         tracing::info!("MQTT Bridge Link closed");
                         break;
                     }
                     Err(e) => {
                         tracing::error!("MQTT Bridge Link error: {}", e);
                         break;
                     }
                 }
             }
        });

        if let Err(e) = broker.start() {
             tracing::error!("MQTT Broker failed: {}", e);
        }
    });
}

mod config {
    use rumqttd::Config;
    
    pub fn create_default_config() -> Config {
        let mut config = Config::default();
        config.id = 0;
        
        // Router config
        config.router = rumqttd::RouterConfig {
            max_connections: 50,
            max_outgoing_packet_count: 100,
            max_segment_size: 1024 * 1024,
            max_segment_count: 10,
            ..Default::default()
        };

        let mut v4_map = std::collections::HashMap::new();
        v4_map.insert("v4".to_string(), rumqttd::ServerSettings {
            name: "v4".to_string(),
            listen: "0.0.0.0:1883".parse().unwrap(),
            tls: None,
            next_connection_delay_ms: 10,
            connections: rumqttd::ConnectionSettings {
                connection_timeout_ms: 100,
                max_payload_size: 20480,
                max_inflight_count: 100,
                auth: None,
                external_auth: None,
                dynamic_filters: true,
            },
        });
        config.v4 = Some(v4_map);
        
        config
    }
}
