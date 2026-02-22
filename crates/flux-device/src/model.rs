use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// 设备 ID（全局唯一）
    pub id: String,
    
    /// 设备名称
    pub name: String,
    
    /// 设备类型
    pub device_type: DeviceType,
    
    /// 通信协议
    pub protocol: Protocol,
    
    /// 设备状态
    pub status: DeviceStatus,
    
    /// 产品 ID
    pub product_id: Option<String>,
    
    /// 设备密钥（加密存储）
    pub secret: Option<String>,
    
    /// 元数据（JSON）
    pub metadata: HashMap<String, String>,
    
    /// 标签
    pub tags: Vec<String>,
    
    /// 所属分组
    pub group_id: Option<String>,
    
    /// 地理位置
    pub location: Option<GeoLocation>,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    
    /// 最后在线时间
    pub last_seen: Option<DateTime<Utc>>,
}

impl Device {
    /// 创建新设备
    pub fn new(name: String, device_type: DeviceType, protocol: Protocol) -> Self {
        let now = Utc::now();
        Self {
            id: format!("dev_{}", uuid::Uuid::new_v4().simple()),
            name,
            device_type,
            protocol,
            status: DeviceStatus::Inactive,
            product_id: None,
            secret: None,
            metadata: HashMap::new(),
            tags: Vec::new(),
            group_id: None,
            location: None,
            created_at: now,
            updated_at: now,
            last_seen: None,
        }
    }

    /// 更新最后在线时间
    pub fn update_last_seen(&mut self) {
        self.last_seen = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// 设置状态
    pub fn set_status(&mut self, status: DeviceStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// 添加标签
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.updated_at = Utc::now();
        }
    }

    /// 移除标签
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
        self.updated_at = Utc::now();
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
        self.updated_at = Utc::now();
    }
}

/// 设备类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DeviceType {
    /// 摄像头
    Camera,
    /// 传感器
    Sensor,
    /// 执行器
    Actuator,
    /// 网关
    Gateway,
    /// 工业设备
    Industrial,
    /// 智能家居
    SmartHome,
    /// 自定义类型
    Custom(String),
}

impl DeviceType {
    pub fn as_str(&self) -> &str {
        match self {
            DeviceType::Camera => "Camera",
            DeviceType::Sensor => "Sensor",
            DeviceType::Actuator => "Actuator",
            DeviceType::Gateway => "Gateway",
            DeviceType::Industrial => "Industrial",
            DeviceType::SmartHome => "SmartHome",
            DeviceType::Custom(s) => s.as_str(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Camera" => DeviceType::Camera,
            "Sensor" => DeviceType::Sensor,
            "Actuator" => DeviceType::Actuator,
            "Gateway" => DeviceType::Gateway,
            "Industrial" => DeviceType::Industrial,
            "SmartHome" => DeviceType::SmartHome,
            _ => DeviceType::Custom(s.to_string()),
        }
    }
}

/// 通信协议
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Protocol {
    MQTT,
    CoAP,
    Modbus,
    OpcUa,
    HTTP,
    RTMP,
    RTSP,
    GB28181,
    ONVIF,
    Custom(String),
}

impl Protocol {
    pub fn as_str(&self) -> &str {
        match self {
            Protocol::MQTT => "MQTT",
            Protocol::CoAP => "CoAP",
            Protocol::Modbus => "Modbus",
            Protocol::OpcUa => "OpcUa",
            Protocol::HTTP => "HTTP",
            Protocol::RTMP => "RTMP",
            Protocol::RTSP => "RTSP",
            Protocol::GB28181 => "GB28181",
            Protocol::ONVIF => "ONVIF",
            Protocol::Custom(s) => s.as_str(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "MQTT" => Protocol::MQTT,
            "CoAP" => Protocol::CoAP,
            "Modbus" => Protocol::Modbus,
            "OpcUa" => Protocol::OpcUa,
            "HTTP" => Protocol::HTTP,
            "RTMP" => Protocol::RTMP,
            "RTSP" => Protocol::RTSP,
            "GB28181" => Protocol::GB28181,
            "ONVIF" => Protocol::ONVIF,
            _ => Protocol::Custom(s.to_string()),
        }
    }
}

/// 设备状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DeviceStatus {
    /// 在线
    Online,
    /// 离线
    Offline,
    /// 故障
    Fault,
    /// 维护中
    Maintenance,
    /// 未激活
    Inactive,
}

impl DeviceStatus {
    pub fn as_str(&self) -> &str {
        match self {
            DeviceStatus::Online => "Online",
            DeviceStatus::Offline => "Offline",
            DeviceStatus::Fault => "Fault",
            DeviceStatus::Maintenance => "Maintenance",
            DeviceStatus::Inactive => "Inactive",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Online" => DeviceStatus::Online,
            "Offline" => DeviceStatus::Offline,
            "Fault" => DeviceStatus::Fault,
            "Maintenance" => DeviceStatus::Maintenance,
            "Inactive" => DeviceStatus::Inactive,
            _ => DeviceStatus::Offline,
        }
    }
}

/// 地理位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    /// 纬度
    pub latitude: f64,
    /// 经度
    pub longitude: f64,
    /// 海拔（米）
    pub altitude: Option<f64>,
    /// 地址
    pub address: Option<String>,
}

/// 设备分组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceGroup {
    /// 分组 ID
    pub id: String,
    
    /// 分组名称
    pub name: String,
    
    /// 分组描述
    pub description: Option<String>,
    
    /// 父分组 ID（支持层级结构）
    pub parent_id: Option<String>,
    
    /// 分组路径（如：/root/building1/floor1）
    pub path: String,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl DeviceGroup {
    /// 创建新分组
    pub fn new(name: String, parent_id: Option<String>) -> Self {
        let now = Utc::now();
        let id = format!("grp_{}", uuid::Uuid::new_v4().simple());
        
        // 构建路径
        let path = if let Some(ref parent) = parent_id {
            format!("/{}/{}", parent, id)
        } else {
            format!("/{}", id)
        };

        Self {
            id,
            name,
            description: None,
            parent_id,
            path,
            created_at: now,
            updated_at: now,
        }
    }
}

/// 设备状态历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatusHistory {
    pub id: i64,
    pub device_id: String,
    pub status: DeviceStatus,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<HashMap<String, String>>,
}

/// 设备指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetrics {
    pub id: i64,
    pub device_id: String,
    pub metric_name: String,
    pub metric_value: f64,
    pub unit: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// 设备过滤器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceFilter {
    /// 设备类型过滤
    pub device_type: Option<DeviceType>,
    /// 协议过滤
    pub protocol: Option<Protocol>,
    /// 状态过滤
    pub status: Option<DeviceStatus>,
    /// 分组过滤
    pub group_id: Option<String>,
    /// 标签过滤（包含任一标签）
    pub tags: Option<Vec<String>>,
    /// 搜索关键词（名称/ID）
    pub search: Option<String>,
    /// 分页：页码
    pub page: Option<u64>,
    /// 分页：每页数量
    pub page_size: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_device() {
        let device = Device::new(
            "测试设备".to_string(),
            DeviceType::Sensor,
            Protocol::MQTT,
        );

        assert!(device.id.starts_with("dev_"));
        assert_eq!(device.name, "测试设备");
        assert_eq!(device.device_type, DeviceType::Sensor);
        assert_eq!(device.protocol, Protocol::MQTT);
        assert_eq!(device.status, DeviceStatus::Inactive);
    }

    #[test]
    fn test_device_tags() {
        let mut device = Device::new(
            "测试设备".to_string(),
            DeviceType::Sensor,
            Protocol::MQTT,
        );

        device.add_tag("temperature".to_string());
        device.add_tag("indoor".to_string());
        assert_eq!(device.tags.len(), 2);

        device.remove_tag("indoor");
        assert_eq!(device.tags.len(), 1);
        assert_eq!(device.tags[0], "temperature");
    }

    #[test]
    fn test_device_type_conversion() {
        assert_eq!(DeviceType::Camera.as_str(), "Camera");
        assert_eq!(DeviceType::from_str("Sensor"), DeviceType::Sensor);
        
        let custom = DeviceType::Custom("MyDevice".to_string());
        assert_eq!(custom.as_str(), "MyDevice");
    }

    #[test]
    fn test_create_device_group() {
        let group = DeviceGroup::new("一楼".to_string(), None);
        assert!(group.id.starts_with("grp_"));
        assert_eq!(group.name, "一楼");
        assert!(group.path.starts_with("/grp_"));
    }
}
