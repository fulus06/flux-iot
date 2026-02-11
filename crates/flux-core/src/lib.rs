pub mod bus;
pub mod entity;
pub mod error;
pub mod service;
pub mod traits;

pub fn init() {
    tracing::info!("Core library initialized");
}
