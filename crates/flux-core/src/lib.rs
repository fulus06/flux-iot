pub mod service;
pub mod entity;
pub mod bus;
pub mod traits;

pub fn init() {
    tracing::info!("Core library initialized");
}
