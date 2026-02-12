// GB28181 设备管理
// 管理已注册的设备信息和状态

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 设备状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    Online,      // 在线
    Offline,     // 离线
    Registering, // 注册中
}

/// GB28181 设备
#[derive(Debug, Clone)]
pub struct Device {
    /// 设备 ID（20位国标编码）
    pub device_id: String,
    
    /// 设备名称
    pub name: String,
    
    /// 设备 IP 地址
    pub ip: String,
    
    /// 设备端口
    pub port: u16,
    
    /// 设备状态
    pub status: DeviceStatus,
    
    /// 注册时间
    pub register_time: DateTime<Utc>,
    
    /// 最后心跳时间
    pub last_keepalive: DateTime<Utc>,
    
    /// 过期时间（秒）
    pub expires: u32,
    
    /// 传输协议（UDP/TCP）
    pub transport: String,
    
    /// 通道列表
    pub channels: Vec<Channel>,
}

impl Device {
    pub fn new(device_id: String, ip: String, port: u16) -> Self {
        let now = Utc::now();
        Self {
            device_id,
            name: String::new(),
            ip,
            port,
            status: DeviceStatus::Registering,
            register_time: now,
            last_keepalive: now,
            expires: 3600, // 默认 1 小时
            transport: "UDP".to_string(),
            channels: Vec::new(),
        }
    }
    
    /// 更新心跳时间
    pub fn update_keepalive(&mut self) {
        self.last_keepalive = Utc::now();
        self.status = DeviceStatus::Online;
    }
    
    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.last_keepalive);
        elapsed.num_seconds() > self.expires as i64
    }
    
    /// 添加通道
    pub fn add_channel(&mut self, channel: Channel) {
        self.channels.push(channel);
    }
    
    /// 获取通道
    pub fn get_channel(&self, channel_id: &str) -> Option<&Channel> {
        self.channels.iter().find(|c| c.channel_id == channel_id)
    }
}

/// 设备通道（摄像头）
#[derive(Debug, Clone)]
pub struct Channel {
    /// 通道 ID（20位国标编码）
    pub channel_id: String,
    
    /// 通道名称
    pub name: String,
    
    /// 制造商
    pub manufacturer: String,
    
    /// 型号
    pub model: String,
    
    /// 通道状态（ON/OFF）
    pub status: String,
    
    /// 父设备 ID
    pub parent_id: String,
    
    /// 经度
    pub longitude: Option<f64>,
    
    /// 纬度
    pub latitude: Option<f64>,
}

impl Channel {
    pub fn new(channel_id: String, name: String, parent_id: String) -> Self {
        Self {
            channel_id,
            name,
            manufacturer: String::new(),
            model: String::new(),
            status: "ON".to_string(),
            parent_id,
            longitude: None,
            latitude: None,
        }
    }
}

/// 设备管理器
pub struct DeviceManager {
    /// 设备列表（device_id -> Device）
    devices: Arc<RwLock<HashMap<String, Device>>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册设备
    pub async fn register_device(&self, device: Device) {
        let device_id = device.device_id.clone();
        let mut devices = self.devices.write().await;
        devices.insert(device_id.clone(), device);
        
        tracing::info!("Device registered: {}", device_id);
    }
    
    /// 注销设备
    pub async fn unregister_device(&self, device_id: &str) -> Option<Device> {
        let mut devices = self.devices.write().await;
        let device = devices.remove(device_id);
        
        if device.is_some() {
            tracing::info!("Device unregistered: {}", device_id);
        }
        
        device
    }
    
    /// 获取设备
    pub async fn get_device(&self, device_id: &str) -> Option<Device> {
        let devices = self.devices.read().await;
        devices.get(device_id).cloned()
    }
    
    /// 更新设备心跳
    pub async fn update_keepalive(&self, device_id: &str) -> bool {
        let mut devices = self.devices.write().await;
        
        if let Some(device) = devices.get_mut(device_id) {
            device.update_keepalive();
            tracing::debug!("Device keepalive updated: {}", device_id);
            true
        } else {
            false
        }
    }
    
    /// 列出所有设备
    pub async fn list_devices(&self) -> Vec<Device> {
        let devices = self.devices.read().await;
        devices.values().cloned().collect()
    }
    
    /// 列出在线设备
    pub async fn list_online_devices(&self) -> Vec<Device> {
        let devices = self.devices.read().await;
        devices
            .values()
            .filter(|d| d.status == DeviceStatus::Online && !d.is_expired())
            .cloned()
            .collect()
    }
    
    /// 清理过期设备
    pub async fn cleanup_expired(&self) -> usize {
        let mut devices = self.devices.write().await;
        let expired: Vec<String> = devices
            .iter()
            .filter(|(_, d)| d.is_expired())
            .map(|(id, _)| id.clone())
            .collect();
        
        let count = expired.len();
        for device_id in expired {
            devices.remove(&device_id);
            tracing::info!("Device expired and removed: {}", device_id);
        }
        
        count
    }
    
    /// 获取设备数量
    pub async fn device_count(&self) -> usize {
        let devices = self.devices.read().await;
        devices.len()
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_device_registration() {
        let manager = DeviceManager::new();
        
        let device = Device::new(
            "34020000001320000001".to_string(),
            "192.168.1.100".to_string(),
            5060,
        );
        
        manager.register_device(device.clone()).await;
        
        let retrieved = manager.get_device("34020000001320000001").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().device_id, "34020000001320000001");
    }
    
    #[tokio::test]
    async fn test_device_keepalive() {
        let manager = DeviceManager::new();
        
        let device = Device::new(
            "34020000001320000001".to_string(),
            "192.168.1.100".to_string(),
            5060,
        );
        
        manager.register_device(device).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let updated = manager.update_keepalive("34020000001320000001").await;
        assert!(updated);
        
        let device = manager.get_device("34020000001320000001").await.unwrap();
        assert_eq!(device.status, DeviceStatus::Online);
    }
    
    #[tokio::test]
    async fn test_device_expiration() {
        let manager = DeviceManager::new();
        
        let mut device = Device::new(
            "34020000001320000001".to_string(),
            "192.168.1.100".to_string(),
            5060,
        );
        
        // 设置很短的过期时间
        device.expires = 1;
        device.last_keepalive = Utc::now() - chrono::Duration::seconds(2);
        
        manager.register_device(device).await;
        
        let count = manager.cleanup_expired().await;
        assert_eq!(count, 1);
        
        let device = manager.get_device("34020000001320000001").await;
        assert!(device.is_none());
    }
    
    #[tokio::test]
    async fn test_list_online_devices() {
        let manager = DeviceManager::new();
        
        // 添加在线设备
        let mut device1 = Device::new(
            "34020000001320000001".to_string(),
            "192.168.1.100".to_string(),
            5060,
        );
        device1.status = DeviceStatus::Online;
        manager.register_device(device1).await;
        
        // 添加离线设备
        let mut device2 = Device::new(
            "34020000001320000002".to_string(),
            "192.168.1.101".to_string(),
            5060,
        );
        device2.status = DeviceStatus::Offline;
        manager.register_device(device2).await;
        
        let online = manager.list_online_devices().await;
        assert_eq!(online.len(), 1);
        assert_eq!(online[0].device_id, "34020000001320000001");
    }
}
