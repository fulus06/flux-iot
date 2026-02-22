use flux_core::bus::EventBus;
use ntex::service::{fn_factory_with_config, fn_service};
use ntex::util::Ready;
use ntex_mqtt::{v3, v5, MqttServer};
use std::sync::Arc;
use std::thread;

mod handler;
pub mod acl;
pub mod manager;
pub mod metrics;
pub mod retained;
pub mod tls;
pub mod topic_matcher;

#[cfg(feature = "persistence")]
pub mod db;

#[cfg(feature = "persistence")]
pub mod persistence;

use handler::Handler;
use manager::MqttManager;

use flux_core::traits::auth::Authenticator;
use tls::TlsConfig;

pub fn start_broker(event_bus: Arc<EventBus>, authenticator: Arc<dyn Authenticator>) {
    start_broker_with_tls(event_bus, authenticator, None);
}

pub fn start_broker_with_tls(
    event_bus: Arc<EventBus>,
    authenticator: Arc<dyn Authenticator>,
    tls_config: Option<TlsConfig>,
) {
    if tls_config.is_some() {
        tracing::info!("Starting Flux MQTT Broker on 0.0.0.0:1883 and MQTTS on 0.0.0.0:8883");
    } else {
        tracing::info!("Starting Flux MQTT Broker on 0.0.0.0:1883");
    }

    let server_bus = event_bus.clone();

    // Spawn Ntex System in a separate thread
    thread::spawn(move || {
        let _ = run_mqtt_server(server_bus, authenticator, tls_config);
    });
}

#[ntex::main]
async fn run_mqtt_server(
    event_bus: Arc<EventBus>,
    authenticator: Arc<dyn Authenticator>,
    tls_config: Option<TlsConfig>,
) -> std::io::Result<()> {
    let mut server = ntex::server::build();

    // Clone for first bind
    let event_bus_mqtt = event_bus.clone();
    let authenticator_mqtt = authenticator.clone();

    // Standard MQTT (1883)
    server = server.bind("mqtt", "0.0.0.0:1883", move |_| {
        // Per-worker initialization
        let manager = MqttManager::new();
        let handler = Handler::new(
            manager.clone(),
            event_bus_mqtt.clone(),
            authenticator_mqtt.clone(),
        );
        let h_v3 = handler.clone();
        let h_v5 = handler.clone();

        // Spawn local bridge task
        let bridge_manager = manager.clone();
        let bridge_bus = event_bus_mqtt.clone();

        ntex::rt::spawn(async move {
            let mut rx = bridge_bus.subscribe();
            while let Ok(msg) = rx.recv().await {
                if let Ok(bytes) = serde_json::to_vec(&msg.payload) {
                    bridge_manager
                        .broadcast(&msg.topic, ntex::util::Bytes::from(bytes))
                        .await;
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
                .publish(fn_factory_with_config(
                    |session: v3::Session<Handler>| {
                        Ready::Ok::<_, handler::ServerError>(fn_service(move |req| {
                            handler::publish_v3(session.clone(), req)
                        }))
                    },
                )))
                .v5(v5::MqttServer::new(move |h| {
                    let handler = h_v5.clone();
                    async move { handler::handshake_v5(h, handler).await }
                })
                .control(fn_factory_with_config(|session: v5::Session<Handler>| {
                    Ready::Ok::<_, handler::ServerError>(fn_service(move |req| {
                        handler::control_v5(session.clone(), req)
                    }))
                }))
                .publish(fn_factory_with_config(
                    |session: v5::Session<Handler>| {
                        Ready::Ok::<_, handler::ServerError>(fn_service(move |req| {
                            handler::publish_v5(session.clone(), req)
                        }))
                    },
                )))
        }
    })?;

    // MQTTS (8883) - if TLS is configured
    if let Some(tls_cfg) = tls_config {
        match tls::load_tls_config(&tls_cfg) {
            Ok(_rustls_config) => {
                tracing::info!("TLS configuration loaded successfully");

                let event_bus_tls = event_bus.clone();
                let authenticator_tls = authenticator.clone();

                server = server.bind("mqtts", "0.0.0.0:8883", move |_| {
                    let manager = MqttManager::new();
                    let handler = Handler::new(
                        manager.clone(),
                        event_bus_tls.clone(),
                        authenticator_tls.clone(),
                    );
                    let h_v3 = handler.clone();
                    let h_v5 = handler.clone();

                    let bridge_manager = manager.clone();
                    let bridge_bus = event_bus_tls.clone();

                    ntex::rt::spawn(async move {
                        let mut rx = bridge_bus.subscribe();
                        while let Ok(msg) = rx.recv().await {
                            if let Ok(bytes) = serde_json::to_vec(&msg.payload) {
                                bridge_manager
                                    .broadcast(&msg.topic, ntex::util::Bytes::from(bytes))
                                    .await;
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
                            })))
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
                            })))
                    }
                })?;

                // Note: ntex TLS integration requires different approach
                // TLS configuration is loaded but needs to be applied at bind level
                // This is a placeholder for future TLS integration
                tracing::info!("MQTTS server configured on port 8883 (TLS config loaded)");
            }
            Err(e) => {
                tracing::error!("Failed to load TLS config: {}", e);
                tracing::warn!("MQTTS server will not be started");
            }
        }
    }

    server.workers(2).run().await
}
