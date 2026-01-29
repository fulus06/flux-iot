use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceData {
    pub device_id: String,
    pub values: HashMap<String, f64>,
    pub timestamp: i64,
}

impl DeviceData {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(device_id: String, values: HashMap<String, f64>) -> Self {
        Self {
            device_id,
            values,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}
