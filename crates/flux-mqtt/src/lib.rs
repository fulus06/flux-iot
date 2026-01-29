use std::sync::Arc;
use std::thread;
use flux_core::bus::EventBus;
use ntex::service::{fn_factory_with_config, fn_service};
use ntex::util::Ready;
use ntex_mqtt::{MqttServer, v3, v5};

mod manager;
mod handler;

use manager::MqttManager;
use handler::Handler;

pub fn start_broker(event_bus: Arc<EventBus>) {
    tracing::info!("Starting Flux MQTT Broker (ntex) on 0.0.0.0:1883");

    let server_bus = event_bus.clone();

    // 1. Spawn Ntex System in a separate thread
    thread::spawn(move || {
        let _ = run_mqtt_server(server_bus);
    });
}

#[ntex::main]
async fn run_mqtt_server(event_bus: Arc<EventBus>) -> std::io::Result<()> {
    
    ntex::server::build()
        .bind("mqtt", "0.0.0.0:1883", move |_| {
            // Per-worker initialization
            let manager = MqttManager::new();
            let handler = Handler::new(manager.clone(), event_bus.clone());
            let h_v3 = handler.clone();
            let h_v5 = handler.clone();
            
            // Spawn local bridge task
            let bridge_manager = manager.clone();
            let bridge_bus = event_bus.clone();
            
            ntex::rt::spawn(async move {
                let mut rx = bridge_bus.subscribe();
                while let Ok(msg) = rx.recv().await {
                    if let Ok(bytes) = serde_json::to_vec(&msg.payload) {
                         bridge_manager.broadcast(&msg.topic, ntex::util::Bytes::from(bytes)).await;
                    }
                }
            });

            async move {
                MqttServer::new()
                    .v3(v3::MqttServer::new(move |h| {
                        let handler = h_v3.clone();
                        async move { handler::handshake_v3(h, handler).await }
                    })
                    .control(fn_factory_with_config(|session: v3::Session<Handler>| {
                        Ready::Ok::<_, handler::ServerError>(fn_service(move |req| {
                            handler::control_v3(session.clone(), req)
                        }))
                    }))
                    .publish(fn_factory_with_config(|session: v3::Session<Handler>| {
                        Ready::Ok::<_, handler::ServerError>(fn_service(move |req| {
                            handler::publish_v3(session.clone(), req)
                        }))
                    }))
                    )
                    .v5(v5::MqttServer::new(move |h| {
                        let handler = h_v5.clone();
                        async move { handler::handshake_v5(h, handler).await }
                    })
                    .control(fn_factory_with_config(|session: v5::Session<Handler>| {
                        Ready::Ok::<_, handler::ServerError>(fn_service(move |req| {
                            handler::control_v5(session.clone(), req)
                        }))
                    }))
                    .publish(fn_factory_with_config(|session: v5::Session<Handler>| {
                        Ready::Ok::<_, handler::ServerError>(fn_service(move |req| {
                            handler::publish_v5(session.clone(), req)
                        }))
                    }))
                    )
            }
        })?
        .workers(2)
        .run()
        .await
}
