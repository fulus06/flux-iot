pub mod trait_def;

#[cfg(feature = "mqtt")]
pub mod mqtt;

pub use trait_def::CommandChannel;

#[cfg(feature = "mqtt")]
pub use mqtt::MqttCommandChannel;

#[cfg(test)]
pub use trait_def::MockCommandChannel;
