use std::rc::Rc;
use std::sync::Arc;
use ntex::service::{fn_factory_with_config, fn_service};
use ntex::util::{Ready, ByteString};
use ntex_mqtt::{v3, v5};
use flux_core::bus::EventBus;
use flux_types::message::Message;
use crate::manager::MqttManager;

#[derive(Clone)]
pub struct Handler {
    manager: MqttManager,
    event_bus: Arc<EventBus>,
    client_id: Option<String>,
}

impl Handler {
    pub fn new(manager: MqttManager, event_bus: Arc<EventBus>) -> Self {
        Self { manager, event_bus, client_id: None }
    }
    
    pub fn with_client_id(&self, client_id: String) -> Self {
        Self {
            manager: self.manager.clone(),
            event_bus: self.event_bus.clone(),
            client_id: Some(client_id),
        }
    }
}

#[derive(Debug)]
pub struct ServerError;

impl From<()> for ServerError {
    fn from(_: ()) -> Self {
        ServerError
    }
}

impl std::convert::TryFrom<ServerError> for v5::PublishAck {
    type Error = ServerError;

    fn try_from(err: ServerError) -> Result<Self, Self::Error> {
        Err(err)
    }
}

// V3 Handlers
pub async fn handshake_v3(
    handshake: v3::Handshake,
    handler: Handler,
) -> Result<v3::HandshakeAck<Handler>, ServerError> {
    let client_id = handshake.packet().client_id.to_string();
    handler.manager.add_v3(client_id.clone(), handshake.sink());
    Ok(handshake.ack(handler.with_client_id(client_id), false))
}

pub async fn control_v3(
    session: v3::Session<Handler>,
    control: v3::Control<ServerError>,
) -> Result<v3::ControlAck, ServerError> {
    match control {
        v3::Control::Protocol(v3::CtlFrame::Subscribe(mut sub)) => {
            for mut s in &mut sub {
               s.subscribe(v3::QoS::AtLeastOnce);
            }
            Ok(sub.ack())
        }
        v3::Control::Protocol(v3::CtlFrame::Unsubscribe(unsub)) => {
            Ok(unsub.ack())
        }
        v3::Control::Protocol(v3::CtlFrame::Disconnect(disc)) => {
             if let Some(id) = &session.state().client_id {
                 session.state().manager.remove(id);
             }
             Ok(disc.ack())
        }
        v3::Control::Flow(v3::CtlFlow::Ping(ping)) => {
            Ok(ping.ack())
        }
        v3::Control::Stop(msg) => {
             if let Some(id) = &session.state().client_id {
                 session.state().manager.remove(id);
             }
             Ok(v3::Control::<ServerError>::Stop(msg).ack())
        }
        v3::Control::Shutdown(msg) => {
             if let Some(id) = &session.state().client_id {
                 session.state().manager.remove(id);
             }
             Ok(v3::Control::<ServerError>::Shutdown(msg).ack())
        }
        other => Ok(other.ack())
    }
}

pub async fn publish_v3(
    session: v3::Session<Handler>,
    mut publish: v3::Publish,
) -> Result<(), ServerError> {
    let topic = publish.topic().path().to_string(); 
    
    // Read payload
    let payload = match publish.take_payload().read_all().await {
        Ok(b) => b,
        Err(_) => return Ok(()), 
    };
    
    // forward to event bus
    let handler = session.state();
    
    // Simple JSON check
    if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&payload) {
        let msg = Message::new(topic.clone(), json_val);
        if let Err(e) = handler.event_bus.publish(msg) {
             tracing::warn!("EventBus publish error: {}", e);
        }
    }
    
    Ok(())
}

// V5 Handlers
pub async fn handshake_v5(
    handshake: v5::Handshake,
    handler: Handler,
) -> Result<v5::HandshakeAck<Handler>, ServerError> {
    let client_id = handshake.packet().client_id.to_string();
    handler.manager.add_v5(client_id.clone(), handshake.sink());
    Ok(handshake.ack(handler.with_client_id(client_id)))
}

pub async fn control_v5(
    session: v5::Session<Handler>,
    control: v5::Control<ServerError>,
) -> Result<v5::ControlAck, ServerError> {
    match control {
        v5::Control::Protocol(v5::CtlFrame::Subscribe(mut sub)) => {
            for mut s in &mut sub {
               s.subscribe(v5::QoS::AtLeastOnce);
            }
            Ok(sub.ack())
        }
        v5::Control::Protocol(v5::CtlFrame::Unsubscribe(unsub)) => {
            Ok(unsub.ack())
        }
        v5::Control::Protocol(v5::CtlFrame::Disconnect(disc)) => {
             if let Some(id) = &session.state().client_id {
                 session.state().manager.remove(id);
             }
             Ok(disc.ack())
        }
        v5::Control::Flow(v5::CtlFlow::Ping(ping)) => {
            Ok(ping.ack())
        }
        // Match Stop/Shutdown to cleanup, but we need to keep the msg to Ack it.
        v5::Control::Stop(msg) => {
             if let Some(id) = &session.state().client_id {
                 session.state().manager.remove(id);
             }
             Ok(v5::Control::<ServerError>::Stop(msg).ack())
        }
        v5::Control::Shutdown(msg) => {
             if let Some(id) = &session.state().client_id {
                 session.state().manager.remove(id);
             }
             Ok(v5::Control::<ServerError>::Shutdown(msg).ack())
        }
        other => Ok(other.ack())
    }
}

pub async fn publish_v5(
    session: v5::Session<Handler>,
    mut publish: v5::Publish,
) -> Result<v5::PublishAck, ServerError> {
    let topic = publish.topic().path().to_string();
    let payload = match publish.take_payload().read_all().await {
        Ok(b) => b,
        Err(_) => return Ok(publish.ack()),
    };
    
    let handler = session.state();
    
    if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&payload) {
         let msg = Message::new(topic.clone(), json_val);
         if let Err(e) = handler.event_bus.publish(msg) {
              tracing::warn!("EventBus publish error: {}", e);
         }
    }
    
    Ok(publish.ack())
}
