use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// ONVIF 客户端
pub struct OnvifClient {
    client: Client,
    service_url: String,
    username: Option<String>,
    password: Option<String>,
}

/// ONVIF Profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnvifProfile {
    pub token: String,
    pub name: String,
    pub video_source_token: String,
    pub video_encoder_token: String,
}

/// ONVIF 媒体 URI
#[derive(Debug, Clone)]
pub struct OnvifMediaUri {
    pub uri: String,
    pub profile_token: String,
}

impl OnvifClient {
    /// 创建 ONVIF 客户端
    pub fn new(service_url: String) -> Self {
        Self {
            client: Client::new(),
            service_url,
            username: None,
            password: None,
        }
    }

    /// 设置认证
    pub fn with_auth(mut self, username: String, password: String) -> Self {
        self.username = Some(username);
        self.password = Some(password);
        self
    }

    /// 获取设备信息
    pub async fn get_device_information(&self) -> Result<DeviceInformation> {
        let request = self.build_soap_request(
            "GetDeviceInformation",
            r#"<GetDeviceInformation xmlns="http://www.onvif.org/ver10/device/wsdl"/>"#,
        );

        let response = self.send_request(&request).await?;
        self.parse_device_information(&response)
    }

    /// 获取 Profiles
    pub async fn get_profiles(&self) -> Result<Vec<OnvifProfile>> {
        let media_url = self.get_media_service_url().await?;
        
        let request = self.build_soap_request(
            "GetProfiles",
            r#"<GetProfiles xmlns="http://www.onvif.org/ver10/media/wsdl"/>"#,
        );

        let response = self.send_request_to(&media_url, &request).await?;
        self.parse_profiles(&response)
    }

    /// 获取流 URI
    pub async fn get_stream_uri(&self, profile_token: &str) -> Result<OnvifMediaUri> {
        let media_url = self.get_media_service_url().await?;
        
        let request = self.build_soap_request(
            "GetStreamUri",
            &format!(
                r#"<GetStreamUri xmlns="http://www.onvif.org/ver10/media/wsdl">
                    <StreamSetup>
                        <Stream xmlns="http://www.onvif.org/ver10/schema">RTP-Unicast</Stream>
                        <Transport xmlns="http://www.onvif.org/ver10/schema">
                            <Protocol>RTSP</Protocol>
                        </Transport>
                    </StreamSetup>
                    <ProfileToken>{}</ProfileToken>
                </GetStreamUri>"#,
                profile_token
            ),
        );

        let response = self.send_request_to(&media_url, &request).await?;
        self.parse_stream_uri(&response, profile_token)
    }

    /// 获取媒体服务 URL
    async fn get_media_service_url(&self) -> Result<String> {
        let request = self.build_soap_request(
            "GetServices",
            r#"<GetServices xmlns="http://www.onvif.org/ver10/device/wsdl">
                <IncludeCapability>false</IncludeCapability>
            </GetServices>"#,
        );

        let response = self.send_request(&request).await?;
        
        // 解析 Media Service URL
        if let Some(start) = response.find("<Namespace>http://www.onvif.org/ver10/media/wsdl</Namespace>") {
            if let Some(xaddr_start) = response[..start].rfind("<XAddr>") {
                let content_start = xaddr_start + 7;
                if let Some(xaddr_end) = response[content_start..].find("</XAddr>") {
                    return Ok(response[content_start..content_start + xaddr_end].to_string());
                }
            }
        }
        
        // 默认使用 /onvif/media_service
        let base_url = self.service_url.trim_end_matches("/onvif/device_service");
        Ok(format!("{}/onvif/media_service", base_url))
    }

    /// 构建 SOAP 请求
    fn build_soap_request(&self, action: &str, body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
  <s:Body>
    {}
  </s:Body>
</s:Envelope>"#,
            body
        )
    }

    /// 发送请求
    async fn send_request(&self, request: &str) -> Result<String> {
        self.send_request_to(&self.service_url, request).await
    }

    /// 发送请求到指定 URL
    async fn send_request_to(&self, url: &str, request: &str) -> Result<String> {
        debug!(target: "onvif_client", "Sending request to {}", url);
        
        let mut req = self.client
            .post(url)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .body(request.to_string());

        // 添加认证
        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            req = req.basic_auth(username, Some(password));
        }

        let response = req.send().await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(anyhow!("ONVIF request failed: {}", status));
        }

        debug!(target: "onvif_client", "Received response");
        Ok(text)
    }

    /// 解析设备信息
    fn parse_device_information(&self, xml: &str) -> Result<DeviceInformation> {
        Ok(DeviceInformation {
            manufacturer: Self::extract_tag(xml, "Manufacturer").unwrap_or_default(),
            model: Self::extract_tag(xml, "Model").unwrap_or_default(),
            firmware_version: Self::extract_tag(xml, "FirmwareVersion").unwrap_or_default(),
            serial_number: Self::extract_tag(xml, "SerialNumber").unwrap_or_default(),
            hardware_id: Self::extract_tag(xml, "HardwareId").unwrap_or_default(),
        })
    }

    /// 解析 Profiles
    fn parse_profiles(&self, xml: &str) -> Result<Vec<OnvifProfile>> {
        let mut profiles = Vec::new();
        
        // 简化的解析（生产环境应使用 quick-xml）
        let mut search_start = 0;
        while let Some(profile_start) = xml[search_start..].find("<trt:Profiles") {
            let abs_start = search_start + profile_start;
            
            if let Some(profile_end) = xml[abs_start..].find("</trt:Profiles>") {
                let profile_xml = &xml[abs_start..abs_start + profile_end + 15];
                
                if let Ok(profile) = self.parse_single_profile(profile_xml) {
                    profiles.push(profile);
                }
                
                search_start = abs_start + profile_end + 15;
            } else {
                break;
            }
        }
        
        Ok(profiles)
    }

    /// 解析单个 Profile
    fn parse_single_profile(&self, xml: &str) -> Result<OnvifProfile> {
        let token = Self::extract_attribute(xml, "token")?;
        let name = Self::extract_tag(xml, "Name").unwrap_or_else(|_| "Unknown".to_string());
        let video_source_token = Self::extract_attribute(xml, "SourceToken").unwrap_or_default();
        let video_encoder_token = Self::extract_attribute(xml, "Encoding").unwrap_or_default();
        
        Ok(OnvifProfile {
            token,
            name,
            video_source_token,
            video_encoder_token,
        })
    }

    /// 解析流 URI
    fn parse_stream_uri(&self, xml: &str, profile_token: &str) -> Result<OnvifMediaUri> {
        let uri = Self::extract_tag(xml, "Uri")?;
        
        Ok(OnvifMediaUri {
            uri,
            profile_token: profile_token.to_string(),
        })
    }

    /// 提取 XML 标签
    fn extract_tag(xml: &str, tag: &str) -> Result<String> {
        let patterns = [
            (format!("<{}>", tag), format!("</{}>", tag)),
            (format!("<tds:{}>", tag), format!("</tds:{}>", tag)),
            (format!("<tt:{}>", tag), format!("</tt:{}>", tag)),
            (format!("<trt:{}>", tag), format!("</trt:{}>", tag)),
        ];
        
        for (start_tag, end_tag) in &patterns {
            if let Some(start) = xml.find(start_tag) {
                let content_start = start + start_tag.len();
                if let Some(end) = xml[content_start..].find(end_tag) {
                    return Ok(xml[content_start..content_start + end].to_string());
                }
            }
        }
        
        Err(anyhow!("Tag {} not found", tag))
    }

    /// 提取 XML 属性
    fn extract_attribute(xml: &str, attr: &str) -> Result<String> {
        let pattern = format!("{}=\"", attr);
        if let Some(start) = xml.find(&pattern) {
            let content_start = start + pattern.len();
            if let Some(end) = xml[content_start..].find('"') {
                return Ok(xml[content_start..content_start + end].to_string());
            }
        }
        Err(anyhow!("Attribute {} not found", attr))
    }
}

/// 设备信息
#[derive(Debug, Clone)]
pub struct DeviceInformation {
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub serial_number: String,
    pub hardware_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tag() {
        let xml = r#"<Manufacturer>Hikvision</Manufacturer>"#;
        let result = OnvifClient::extract_tag(xml, "Manufacturer").unwrap();
        assert_eq!(result, "Hikvision");
    }

    #[test]
    fn test_extract_attribute() {
        let xml = r#"<Profile token="Profile_1">"#;
        let result = OnvifClient::extract_attribute(xml, "token").unwrap();
        assert_eq!(result, "Profile_1");
    }

    #[test]
    fn test_build_soap_request() {
        let client = OnvifClient::new("http://192.168.1.100/onvif/device_service".to_string());
        let request = client.build_soap_request("GetDeviceInformation", "<Test/>");
        
        assert!(request.contains("Envelope"));
        assert!(request.contains("<Test/>"));
    }
}
