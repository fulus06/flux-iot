use crate::{Device, DeviceGroup, DeviceMetrics, DeviceStatus, DeviceStatusHistory, DeviceType, Protocol};
use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::Set;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Device 模型与数据库实体的转换
impl From<Device> for super::device::ActiveModel {
    fn from(device: Device) -> Self {
        Self {
            id: Set(device.id),
            name: Set(device.name),
            device_type: Set(device.device_type.as_str().to_string()),
            protocol: Set(device.protocol.as_str().to_string()),
            status: Set(device.status.as_str().to_string()),
            product_id: Set(device.product_id),
            secret: Set(device.secret),
            metadata: Set(metadata_to_json(&device.metadata)),
            tags: Set(tags_to_json(&device.tags)),
            group_id: Set(device.group_id),
            location: Set(location_to_json(device.location.as_ref())),
            created_at: Set(device.created_at),
            updated_at: Set(device.updated_at),
            last_seen: Set(device.last_seen),
        }
    }
}

impl From<super::device::Model> for Device {
    fn from(model: super::device::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            device_type: DeviceType::from_str(&model.device_type),
            protocol: Protocol::from_str(&model.protocol),
            status: DeviceStatus::from_str(&model.status),
            product_id: model.product_id,
            secret: model.secret,
            metadata: json_to_metadata(model.metadata.as_ref()),
            tags: json_to_tags(model.tags.as_ref()),
            group_id: model.group_id,
            location: json_to_location(model.location.as_ref()),
            created_at: model.created_at,
            updated_at: model.updated_at,
            last_seen: model.last_seen,
        }
    }
}

/// DeviceGroup 模型与数据库实体的转换
impl From<DeviceGroup> for super::device_group::ActiveModel {
    fn from(group: DeviceGroup) -> Self {
        Self {
            id: Set(group.id),
            name: Set(group.name),
            description: Set(group.description),
            parent_id: Set(group.parent_id),
            path: Set(group.path),
            created_at: Set(group.created_at),
            updated_at: Set(group.updated_at),
        }
    }
}

impl From<super::device_group::Model> for DeviceGroup {
    fn from(model: super::device_group::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            parent_id: model.parent_id,
            path: model.path,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// DeviceStatusHistory 模型与数据库实体的转换
impl From<DeviceStatusHistory> for super::device_status_history::ActiveModel {
    fn from(history: DeviceStatusHistory) -> Self {
        use sea_orm::ActiveValue::NotSet;
        Self {
            id: if history.id == 0 { NotSet } else { Set(history.id) },
            device_id: Set(history.device_id),
            status: Set(history.status.as_str().to_string()),
            timestamp: Set(history.timestamp),
            metadata: Set(metadata_to_json(&history.metadata.unwrap_or_default())),
        }
    }
}

impl From<super::device_status_history::Model> for DeviceStatusHistory {
    fn from(model: super::device_status_history::Model) -> Self {
        Self {
            id: model.id,
            device_id: model.device_id,
            status: DeviceStatus::from_str(&model.status),
            timestamp: model.timestamp,
            metadata: Some(json_to_metadata(model.metadata.as_ref())),
        }
    }
}

/// DeviceMetrics 模型与数据库实体的转换
impl From<DeviceMetrics> for super::device_metrics::ActiveModel {
    fn from(metrics: DeviceMetrics) -> Self {
        use sea_orm::ActiveValue::NotSet;
        Self {
            id: if metrics.id == 0 { NotSet } else { Set(metrics.id) },
            device_id: Set(metrics.device_id),
            metric_name: Set(metrics.metric_name),
            metric_value: Set(metrics.metric_value),
            unit: Set(metrics.unit),
            timestamp: Set(metrics.timestamp),
        }
    }
}

impl From<super::device_metrics::Model> for DeviceMetrics {
    fn from(model: super::device_metrics::Model) -> Self {
        Self {
            id: model.id,
            device_id: model.device_id,
            metric_name: model.metric_name,
            metric_value: model.metric_value,
            unit: model.unit,
            timestamp: model.timestamp,
        }
    }
}

// ========== 辅助函数 ==========

/// 将 HashMap 转换为 JSONB
fn metadata_to_json(metadata: &HashMap<String, String>) -> Option<JsonValue> {
    if metadata.is_empty() {
        None
    } else {
        serde_json::to_value(metadata).ok()
    }
}

/// 将 JSONB 转换为 HashMap
fn json_to_metadata(json: Option<&JsonValue>) -> HashMap<String, String> {
    json.and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// 将 GeoLocation 转换为 JSONB
fn location_to_json(location: Option<&crate::GeoLocation>) -> Option<JsonValue> {
    location.and_then(|loc| serde_json::to_value(loc).ok())
}

/// 将 JSONB 转换为 GeoLocation
fn json_to_location(json: Option<&JsonValue>) -> Option<crate::GeoLocation> {
    json.and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// 将 Vec<String> 转换为 JSON
fn tags_to_json(tags: &[String]) -> Option<JsonValue> {
    if tags.is_empty() {
        None
    } else {
        serde_json::to_value(tags).ok()
    }
}

/// 将 JSON 转换为 Vec<String>
fn json_to_tags(json: Option<&JsonValue>) -> Vec<String> {
    json.and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_device_conversion() {
        let device = Device::new(
            "测试设备".to_string(),
            DeviceType::Sensor,
            Protocol::MQTT,
        );

        // Device -> ActiveModel
        let active_model: crate::db::device::ActiveModel = device.clone().into();
        assert_eq!(active_model.name.unwrap(), "测试设备");
        assert_eq!(active_model.device_type.unwrap(), "Sensor");
        assert_eq!(active_model.protocol.unwrap(), "MQTT");

        // Model -> Device (需要先创建 Model)
        let model = crate::db::device::Model {
            id: device.id.clone(),
            name: device.name.clone(),
            device_type: "Sensor".to_string(),
            protocol: "MQTT".to_string(),
            status: "Inactive".to_string(),
            product_id: None,
            secret: None,
            metadata: None,
            tags: None,
            group_id: None,
            location: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_seen: None,
        };

        let converted: Device = model.into();
        assert_eq!(converted.name, "测试设备");
        assert_eq!(converted.device_type, DeviceType::Sensor);
        assert_eq!(converted.protocol, Protocol::MQTT);
    }

    #[test]
    fn test_metadata_conversion() {
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());
        metadata.insert("key2".to_string(), "value2".to_string());

        // HashMap -> JSON
        let json = metadata_to_json(&metadata);
        assert!(json.is_some());

        // JSON -> HashMap
        let converted = json_to_metadata(json.as_ref());
        assert_eq!(converted.len(), 2);
        assert_eq!(converted.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_location_conversion() {
        let location = crate::GeoLocation {
            latitude: 39.9042,
            longitude: 116.4074,
            altitude: Some(50.0),
            address: Some("北京市".to_string()),
        };

        // GeoLocation -> JSON
        let json = location_to_json(Some(&location));
        assert!(json.is_some());

        // JSON -> GeoLocation
        let converted = json_to_location(json.as_ref());
        assert!(converted.is_some());
        let loc = converted.unwrap();
        assert_eq!(loc.latitude, 39.9042);
        assert_eq!(loc.longitude, 116.4074);
    }

    #[test]
    fn test_device_group_conversion() {
        let group = DeviceGroup::new("测试分组".to_string(), None);

        // DeviceGroup -> ActiveModel
        let active_model: crate::db::device_group::ActiveModel = group.clone().into();
        assert_eq!(active_model.name.unwrap(), "测试分组");

        // Model -> DeviceGroup
        let model = crate::db::device_group::Model {
            id: group.id.clone(),
            name: group.name.clone(),
            description: None,
            parent_id: None,
            path: group.path.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let converted: DeviceGroup = model.into();
        assert_eq!(converted.name, "测试分组");
    }
}
