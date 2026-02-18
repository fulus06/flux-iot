// GB28181 目录查询
// 解析和处理设备目录 XML 消息

use quick_xml::de::from_str;
use serde::Deserialize;
use crate::Result;

/// GB28181 XML 消息根节点
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GB28181XmlMessage {
    /// 命令类型
    pub cmd_type: String,
    
    /// 序列号
    #[serde(rename = "SN")]
    pub sn: Option<u32>,
    
    /// 设备 ID
    #[serde(rename = "DeviceID")]
    pub device_id: String,
    
    /// 目录内容（仅当 CmdType=Catalog 时存在）
    #[serde(default)]
    pub sum_num: Option<u32>,
    
    /// 设备列表
    #[serde(default)]
    pub device_list: Option<DeviceList>,

    /// 设备名称（DeviceInfo）
    #[serde(rename = "DeviceName", default)]
    pub device_name: String,

    /// 制造商（DeviceInfo）
    #[serde(default)]
    pub manufacturer: String,

    /// 型号（DeviceInfo）
    #[serde(default)]
    pub model: String,

    /// 固件版本（DeviceInfo）
    #[serde(rename = "Firmware", default)]
    pub firmware: String,

    /// 结果（DeviceInfo/DeviceStatus）
    #[serde(default)]
    pub result: String,

    /// 在线状态（DeviceStatus）
    #[serde(rename = "Online", default)]
    pub online_status: String,

    /// 设备状态（DeviceStatus，部分厂商使用 Status 表示 ONLINE/OFFLINE）
    #[serde(rename = "Status", default)]
    pub device_status: String,
}

/// 设备列表
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct DeviceList {
    /// 列表数量（作为属性）
    #[serde(rename = "@Num", default)]
    pub num: Option<u32>,
    
    /// 设备项列表
    #[serde(rename = "Item", default)]
    pub items: Vec<DeviceItem>,
}

/// 设备项（通道信息）
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct DeviceItem {
    /// 设备/通道 ID
    #[serde(rename = "DeviceID")]
    pub device_id: String,
    
    /// 设备/通道名称
    #[serde(default)]
    pub name: String,
    
    /// 制造商
    #[serde(default)]
    pub manufacturer: String,
    
    /// 型号
    #[serde(default)]
    pub model: String,
    
    /// 设备归属
    #[serde(default)]
    pub owner: String,
    
    /// 行政区域
    #[serde(default)]
    pub civil_code: String,
    
    /// 安装地址
    #[serde(default)]
    pub address: String,
    
    /// 是否有子设备（1-有，0-没有）
    #[serde(default)]
    pub parental: u8,
    
    /// 父设备 ID
    #[serde(rename = "ParentID", default)]
    pub parent_id: String,
    
    /// 安全方式
    #[serde(default)]
    pub safety_way: u8,
    
    /// 注册方式
    #[serde(default)]
    pub register_way: u8,
    
    /// 保密属性
    #[serde(default)]
    pub secrecy: u8,
    
    /// 状态（ON/OFF）
    #[serde(default)]
    pub status: String,
    
    /// 经度
    #[serde(default)]
    pub longitude: Option<f64>,
    
    /// 纬度
    #[serde(default)]
    pub latitude: Option<f64>,
}

/// 目录查询请求（发送给设备）
#[derive(Debug)]
pub struct CatalogQuery {
    pub sn: u32,
    pub device_id: String,
}

impl CatalogQuery {
    pub fn new(sn: u32, device_id: String) -> Self {
        Self { sn, device_id }
    }
    
    /// 生成目录查询 XML
    pub fn to_xml(&self) -> String {
        format!(
            r#"<?xml version="1.0" encoding="GB2312"?>
<Query>
<CmdType>Catalog</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
            self.sn, self.device_id
        )
    }
}

/// 解析 GB28181 XML 消息
pub fn parse_gb28181_xml(xml: &str) -> Result<GB28181XmlMessage> {
    // 移除 XML 声明（quick-xml 会自动处理）
    let xml = xml.trim();
    
    from_str(xml).map_err(|e| {
        crate::VideoError::Other(format!("Failed to parse GB28181 XML: {}", e))
    })
}

/// 判断是否为目录响应
pub fn is_catalog_response(xml: &str) -> bool {
    xml.contains("<CmdType>Catalog</CmdType>")
}

/// 判断是否为心跳消息
pub fn is_keepalive(xml: &str) -> bool {
    xml.contains("<CmdType>Keepalive</CmdType>")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_catalog_response() {
        let xml = r#"<?xml version="1.0" encoding="GB2312"?>
<Response>
<CmdType>Catalog</CmdType>
<SN>123</SN>
<DeviceID>34020000001320000001</DeviceID>
<SumNum>2</SumNum>
<DeviceList Num="2">
<Item>
<DeviceID>34020000001320000001</DeviceID>
<Name>摄像头1</Name>
<Manufacturer>海康威视</Manufacturer>
<Model>DS-2CD3T46WD</Model>
<Owner>Owner</Owner>
<CivilCode>CivilCode</CivilCode>
<Address>Address</Address>
<Parental>0</Parental>
<ParentID>34020000001320000001</ParentID>
<SafetyWay>0</SafetyWay>
<RegisterWay>1</RegisterWay>
<Secrecy>0</Secrecy>
<Status>ON</Status>
</Item>
<Item>
<DeviceID>34020000001320000002</DeviceID>
<Name>摄像头2</Name>
<Manufacturer>大华</Manufacturer>
<Model>DH-IPC-HFW</Model>
<Owner>Owner</Owner>
<CivilCode>CivilCode</CivilCode>
<Address>Address</Address>
<Parental>0</Parental>
<ParentID>34020000001320000001</ParentID>
<SafetyWay>0</SafetyWay>
<RegisterWay>1</RegisterWay>
<Secrecy>0</Secrecy>
<Status>ON</Status>
</Item>
</DeviceList>
</Response>"#;
        
        let msg = parse_gb28181_xml(xml).unwrap();
        
        assert_eq!(msg.cmd_type, "Catalog");
        assert_eq!(msg.sn, Some(123));
        assert_eq!(msg.device_id, "34020000001320000001");
        assert_eq!(msg.sum_num, Some(2));
        
        let device_list = msg.device_list.unwrap();
        assert_eq!(device_list.num, Some(2));
        assert_eq!(device_list.items.len(), 2);
        
        let item1 = &device_list.items[0];
        assert_eq!(item1.device_id, "34020000001320000001");
        assert_eq!(item1.name, "摄像头1");
        assert_eq!(item1.manufacturer, "海康威视");
        assert_eq!(item1.model, "DS-2CD3T46WD");
        assert_eq!(item1.status, "ON");
        
        let item2 = &device_list.items[1];
        assert_eq!(item2.device_id, "34020000001320000002");
        assert_eq!(item2.name, "摄像头2");
        assert_eq!(item2.manufacturer, "大华");
    }
    
    #[test]
    fn test_catalog_query_xml() {
        let query = CatalogQuery::new(456, "34020000001320000001".to_string());
        let xml = query.to_xml();
        
        assert!(xml.contains("<CmdType>Catalog</CmdType>"));
        assert!(xml.contains("<SN>456</SN>"));
        assert!(xml.contains("<DeviceID>34020000001320000001</DeviceID>"));
    }
    
    #[test]
    fn test_is_catalog_response() {
        let xml = r#"<Response><CmdType>Catalog</CmdType></Response>"#;
        assert!(is_catalog_response(xml));
        
        let xml2 = r#"<Response><CmdType>Keepalive</CmdType></Response>"#;
        assert!(!is_catalog_response(xml2));
    }
    
    #[test]
    fn test_is_keepalive() {
        let xml = r#"<Notify><CmdType>Keepalive</CmdType></Notify>"#;
        assert!(is_keepalive(xml));
        
        let xml2 = r#"<Response><CmdType>Catalog</CmdType></Response>"#;
        assert!(!is_keepalive(xml2));
    }
    
    #[test]
    fn test_parse_minimal_catalog() {
        let xml = r#"<?xml version="1.0"?>
<Response>
<CmdType>Catalog</CmdType>
<SN>1</SN>
<DeviceID>34020000001320000001</DeviceID>
<SumNum>0</SumNum>
</Response>"#;
        
        let msg = parse_gb28181_xml(xml).unwrap();
        assert_eq!(msg.cmd_type, "Catalog");
        assert_eq!(msg.sum_num, Some(0));
        assert!(msg.device_list.is_none());
    }
}
