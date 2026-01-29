use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceData {
    pub device_id: String,
    pub values: HashMap<String, f64>,
    pub timestamp: i64,
}

impl DeviceData {
    pub fn new(device_id: impl Into<String>, values: HashMap<String, f64>) -> Self {
        Self {
            device_id: device_id.into(),
            values,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}
