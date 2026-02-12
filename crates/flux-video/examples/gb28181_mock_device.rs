// GB28181 模拟设备：REGISTER + INVITE 응答 + RTP/PS 推流

use bytes::Bytes;
use flux_video::gb28181::sip::{
    parse_gb28181_xml,
    SdpSession,
    SipMessage,
    SipMethod,
    SipRequest,
    SipResponse,
};
use flux_video::{Result, VideoError};
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Debug, Clone)]
struct MockDeviceConfig {
    device_id: String,
    domain: String,
    local_ip: String,
    local_port: u16,
    server_ip: String,
    server_port: u16,
    ssrc: u32,
    channel_count: u32,
    channel_name_prefix: String,
    channel_status: String,
    longitude: Option<f64>,
    latitude: Option<f64>,
    keepalive_interval: u64,
}

impl Default for MockDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: "34020000001320000001".to_string(),
            domain: "3402000000".to_string(),
            local_ip: "127.0.0.1".to_string(),
            local_port: 5062,
            server_ip: "127.0.0.1".to_string(),
            server_port: 5060,
            ssrc: 3402000001,
            channel_count: 1,
            channel_name_prefix: "MockCamera".to_string(),
            channel_status: "ON".to_string(),
            longitude: None,
            latitude: None,
            keepalive_interval: 30,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let config = parse_args();
    tracing::info!("GB28181 mock device config: {:?}", config);

    let bind_addr: SocketAddr = format!("{}:{}", config.local_ip, config.local_port)
        .parse()
        .map_err(|e| VideoError::Other(format!("Invalid bind addr: {}", e)))?;

    let server_addr: SocketAddr = format!("{}:{}", config.server_ip, config.server_port)
        .parse()
        .map_err(|e| VideoError::Other(format!("Invalid server addr: {}", e)))?;

    let socket = Arc::new(
        UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| VideoError::Other(format!("Bind UDP failed: {}", e)))?,
    );

    send_register(&socket, &config, server_addr).await?;

    // 启动 Keepalive 心跳循环
    let ka_socket = Arc::clone(&socket);
    let ka_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = keepalive_loop(ka_socket, ka_config, server_addr).await {
            tracing::error!("Keepalive loop stopped: {}", e);
        }
    });

    let mut buf = vec![0u8; 65536];
    loop {
        let (len, addr) = socket
            .recv_from(&mut buf)
            .await
            .map_err(|e| VideoError::Other(format!("UDP recv failed: {}", e)))?;
        let msg = String::from_utf8_lossy(&buf[..len]);
        tracing::info!("Device received SIP from {}: {}", addr, msg);

        let sip = SipMessage::from_string(&msg)
            .map_err(|e| VideoError::Other(format!("Parse SIP failed: {}", e)))?;

        match sip {
            SipMessage::Request(req) => {
                match req.method {
                    SipMethod::Invite => {
                        handle_invite(&socket, &config, &req, addr).await?;
                    }
                    SipMethod::Ack => {
                        tracing::info!("Received ACK");
                    }
                    SipMethod::Message => {
                        if let Some(body) = &req.body {
                            if body.contains("<CmdType>Catalog</CmdType>") {
                                handle_catalog_query(&socket, &config, body, server_addr).await?;
                            }
                        }
                        send_basic_ok(&socket, &req, addr).await?;
                    }
                    _ => {
                        send_basic_ok(&socket, &req, addr).await?;
                    }
                }
            }
            SipMessage::Response(resp) => {
                tracing::info!("Received SIP response: {} {}", resp.status_code, resp.reason_phrase);
            }
        }
    }
}

fn parse_args() -> MockDeviceConfig {
    let mut config = MockDeviceConfig::default();
    let args: Vec<String> = env::args().collect();

    for i in 1..args.len() {
        match args[i].as_str() {
            "--device-id" => if let Some(v) = args.get(i + 1) { config.device_id = v.clone(); },
            "--domain" => if let Some(v) = args.get(i + 1) { config.domain = v.clone(); },
            "--local-ip" => if let Some(v) = args.get(i + 1) { config.local_ip = v.clone(); },
            "--local-port" => if let Some(v) = args.get(i + 1) { config.local_port = v.parse().unwrap_or(config.local_port); },
            "--server-ip" => if let Some(v) = args.get(i + 1) { config.server_ip = v.clone(); },
            "--server-port" => if let Some(v) = args.get(i + 1) { config.server_port = v.parse().unwrap_or(config.server_port); },
            "--ssrc" => if let Some(v) = args.get(i + 1) { config.ssrc = v.parse().unwrap_or(config.ssrc); },
            "--channel-count" => if let Some(v) = args.get(i + 1) { config.channel_count = v.parse().unwrap_or(config.channel_count); },
            "--channel-name-prefix" => if let Some(v) = args.get(i + 1) { config.channel_name_prefix = v.clone(); },
            "--channel-status" => if let Some(v) = args.get(i + 1) { config.channel_status = v.clone(); },
            "--longitude" => if let Some(v) = args.get(i + 1) { config.longitude = v.parse().ok(); },
            "--latitude" => if let Some(v) = args.get(i + 1) { config.latitude = v.parse().ok(); },
            "--keepalive-interval" => if let Some(v) = args.get(i + 1) { config.keepalive_interval = v.parse().unwrap_or(config.keepalive_interval); },
            _ => {}
        }
    }

    config
}

async fn send_register(socket: &UdpSocket, cfg: &MockDeviceConfig, server_addr: SocketAddr) -> Result<()> {
    let mut req = SipRequest::new(
        SipMethod::Register,
        format!("sip:{}@{}", cfg.device_id, cfg.domain),
    );

    let call_id = format!("{}@{}", chrono::Utc::now().timestamp(), cfg.domain);
    let branch = format!("z9hG4bK{}", chrono::Utc::now().timestamp());

    req.add_header(
        "Via".to_string(),
        format!("SIP/2.0/UDP {}:{};branch={}", cfg.local_ip, cfg.local_port, branch),
    );
    req.add_header(
        "From".to_string(),
        format!("<sip:{}@{}>;tag=1", cfg.device_id, cfg.domain),
    );
    req.add_header(
        "To".to_string(),
        format!("<sip:{}@{}>", cfg.device_id, cfg.domain),
    );
    req.add_header("Call-ID".to_string(), call_id);
    req.add_header("CSeq".to_string(), "1 REGISTER".to_string());
    req.add_header(
        "Contact".to_string(),
        format!("<sip:{}@{}:{}>", cfg.device_id, cfg.local_ip, cfg.local_port),
    );
    req.add_header("Max-Forwards".to_string(), "70".to_string());
    req.add_header("Expires".to_string(), "3600".to_string());

    socket
        .send_to(req.to_string().as_bytes(), server_addr)
        .await
        .map_err(|e| VideoError::Other(format!("Send REGISTER failed: {}", e)))?;

    tracing::info!("REGISTER sent to {}", server_addr);
    Ok(())
}

async fn keepalive_loop(socket: Arc<UdpSocket>, cfg: MockDeviceConfig, server_addr: SocketAddr) -> Result<()> {
    let mut sn: u32 = 1;

    loop {
        send_keepalive(&socket, &cfg, server_addr, sn).await?;
        sn = sn.wrapping_add(1);
        tokio::time::sleep(tokio::time::Duration::from_secs(cfg.keepalive_interval)).await;
    }
}

async fn send_keepalive(
    socket: &UdpSocket,
    cfg: &MockDeviceConfig,
    server_addr: SocketAddr,
    sn: u32,
) -> Result<()> {
    let mut req = SipRequest::new(
        SipMethod::Message,
        format!("sip:{}@{}", cfg.device_id, cfg.domain),
    );

    let call_id = format!("{}@{}", chrono::Utc::now().timestamp(), cfg.domain);
    let branch = format!("z9hG4bK{}", chrono::Utc::now().timestamp());

    req.add_header(
        "Via".to_string(),
        format!("SIP/2.0/UDP {}:{};branch={}", cfg.local_ip, cfg.local_port, branch),
    );
    req.add_header(
        "From".to_string(),
        format!("<sip:{}@{}>;tag=1", cfg.device_id, cfg.domain),
    );
    req.add_header(
        "To".to_string(),
        format!("<sip:{}@{}>", cfg.device_id, cfg.domain),
    );
    req.add_header("Call-ID".to_string(), call_id);
    req.add_header("CSeq".to_string(), format!("{} MESSAGE", sn));
    req.add_header("Max-Forwards".to_string(), "70".to_string());
    req.add_header(
        "Content-Type".to_string(),
        "Application/MANSCDP+xml".to_string(),
    );

    let xml = format!(
        r#"<?xml version="1.0" encoding="GB2312"?>
<Notify>
<CmdType>Keepalive</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<Status>OK</Status>
</Notify>"#,
        sn, cfg.device_id
    );

    req.set_body(xml);

    socket
        .send_to(req.to_string().as_bytes(), server_addr)
        .await
        .map_err(|e| VideoError::Other(format!("Send Keepalive failed: {}", e)))?;

    tracing::debug!("Keepalive sent to {} (SN={})", server_addr, sn);
    Ok(())
}

async fn handle_invite(socket: &UdpSocket, cfg: &MockDeviceConfig, req: &SipRequest, addr: SocketAddr) -> Result<()> {
    tracing::info!("Handling INVITE from {}", addr);

    let sdp = req.body.clone().unwrap_or_default();
    let target = parse_invite_target(&sdp, addr.ip(), cfg.local_port)?;

    let mut resp = SipResponse::new(200, "OK".to_string());
    copy_headers(req, &mut resp);

    let mut sdp_session = SdpSession::new(cfg.device_id.clone(), cfg.local_ip.clone());
    sdp_session.add_video(cfg.local_port + 2);
    resp.set_body(sdp_session.to_string());
    resp.add_header("Content-Type".to_string(), "application/sdp".to_string());

    socket
        .send_to(resp.to_string().as_bytes(), addr)
        .await
        .map_err(|e| VideoError::Other(format!("Send INVITE OK failed: {}", e)))?;

    tracing::info!("INVITE 200 OK sent, start RTP -> {}", target);
    spawn_rtp_sender(cfg.ssrc, target).await;
    Ok(())
}

async fn handle_catalog_query(
    socket: &UdpSocket,
    cfg: &MockDeviceConfig,
    body: &str,
    server_addr: SocketAddr,
) -> Result<()> {
    let msg = parse_gb28181_xml(body)
        .map_err(|e| VideoError::Other(format!("Parse catalog XML failed: {}", e)))?;

    let sn = msg.sn.unwrap_or(1);
    let xml = build_catalog_response_xml(cfg, sn);

    let mut req = SipRequest::new(
        SipMethod::Message,
        format!("sip:{}@{}", cfg.device_id, cfg.domain),
    );

    let call_id = format!("{}@{}", chrono::Utc::now().timestamp(), cfg.domain);
    let branch = format!("z9hG4bK{}", chrono::Utc::now().timestamp());

    req.add_header(
        "Via".to_string(),
        format!("SIP/2.0/UDP {}:{};branch={}", cfg.local_ip, cfg.local_port, branch),
    );
    req.add_header(
        "From".to_string(),
        format!("<sip:{}@{}>;tag=1", cfg.device_id, cfg.domain),
    );
    req.add_header(
        "To".to_string(),
        format!("<sip:{}@{}>", cfg.device_id, cfg.domain),
    );
    req.add_header("Call-ID".to_string(), call_id);
    req.add_header("CSeq".to_string(), format!("{} MESSAGE", sn));
    req.add_header(
        "Content-Type".to_string(),
        "Application/MANSCDP+xml".to_string(),
    );
    req.add_header("Max-Forwards".to_string(), "70".to_string());

    req.set_body(xml);

    socket
        .send_to(req.to_string().as_bytes(), server_addr)
        .await
        .map_err(|e| VideoError::Other(format!("Send catalog response failed: {}", e)))?;

    tracing::info!("Catalog response sent to {} (SN={})", server_addr, sn);
    Ok(())
}

async fn send_basic_ok(socket: &UdpSocket, req: &SipRequest, addr: SocketAddr) -> Result<()> {
    let mut resp = SipResponse::new(200, "OK".to_string());
    copy_headers(req, &mut resp);
    socket
        .send_to(resp.to_string().as_bytes(), addr)
        .await
        .map_err(|e| VideoError::Other(format!("Send 200 OK failed: {}", e)))?;
    Ok(())
}

fn copy_headers(req: &SipRequest, resp: &mut SipResponse) {
    for key in ["Via", "From", "To", "Call-ID", "CSeq"] {
        if let Some(value) = req.headers.get(key) {
            resp.add_header(key.to_string(), value.clone());
        }
    }
}

fn parse_invite_target(sdp: &str, default_ip: IpAddr, default_port: u16) -> Result<SocketAddr> {
    let session = SdpSession::from_string(sdp)?;
    let ip = session.connection.address.parse::<IpAddr>()
        .unwrap_or(default_ip);
    let port = session.media.first().map(|m| m.port).unwrap_or(default_port);
    Ok(SocketAddr::new(ip, port))
}

fn build_catalog_response_xml(cfg: &MockDeviceConfig, sn: u32) -> String {
    let count = if cfg.channel_count == 0 {
        1
    } else {
        cfg.channel_count
    };

    let mut items = String::new();

    for i in 0..count {
        let channel_id = format!("{}{:03}", cfg.device_id, i + 1);
        let name = format!("{}{}", cfg.channel_name_prefix, i + 1);

        let longitude_xml = cfg
            .longitude
            .map(|lon| format!("<Longitude>{:.6}</Longitude>", lon))
            .unwrap_or_else(String::new);
        let latitude_xml = cfg
            .latitude
            .map(|lat| format!("<Latitude>{:.6}</Latitude>", lat))
            .unwrap_or_else(String::new);

        items.push_str(&format!(
            "<Item>\n\
<DeviceID>{}</DeviceID>\n\
<Name>{}</Name>\n\
<Manufacturer>MockVendor</Manufacturer>\n\
<Model>MockModel</Model>\n\
<Owner>MockOwner</Owner>\n\
<CivilCode>MockCode</CivilCode>\n\
<Address>MockAddress</Address>\n\
<Parental>0</Parental>\n\
<ParentID>{}</ParentID>\n\
<SafetyWay>0</SafetyWay>\n\
<RegisterWay>1</RegisterWay>\n\
<Secrecy>0</Secrecy>\n\
<Status>{}</Status>\n\
{}\n\
{}\n\
</Item>\n",
            channel_id,
            name,
            cfg.device_id,
            cfg.channel_status,
            longitude_xml,
            latitude_xml,
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="GB2312"?>
<Response>
<CmdType>Catalog</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<SumNum>{}</SumNum>
<DeviceList Num="{}">
{}
</DeviceList>
</Response>"#,
        sn, cfg.device_id, count, count, items
    )
}

async fn spawn_rtp_sender(ssrc: u32, target: SocketAddr) {
    tokio::spawn(async move {
        if let Err(e) = rtp_send_loop(ssrc, target).await {
            tracing::error!("RTP sender stopped: {}", e);
        }
    });
}

async fn rtp_send_loop(ssrc: u32, target: SocketAddr) -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await
        .map_err(|e| VideoError::Other(format!("Bind RTP socket failed: {}", e)))?;

    let mut sequence: u16 = 1;
    let mut timestamp: u32 = 0;

    loop {
        let ps = build_ps_payload();
        let rtp = build_rtp_packet(ssrc, sequence, timestamp, true, ps);

        socket
            .send_to(&rtp, target)
            .await
            .map_err(|e| VideoError::Other(format!("Send RTP failed: {}", e)))?;

        sequence = sequence.wrapping_add(1);
        timestamp = timestamp.wrapping_add(3600);
        tokio::time::sleep(tokio::time::Duration::from_millis(40)).await;
    }
}

fn build_rtp_packet(ssrc: u32, sequence: u16, timestamp: u32, marker: bool, payload: Bytes) -> Vec<u8> {
    let mut packet = Vec::with_capacity(12 + payload.len());
    let m_pt = if marker { 0x80 | 96 } else { 96 };

    packet.push(0x80);
    packet.push(m_pt);
    packet.extend_from_slice(&sequence.to_be_bytes());
    packet.extend_from_slice(&timestamp.to_be_bytes());
    packet.extend_from_slice(&ssrc.to_be_bytes());
    packet.extend_from_slice(&payload);

    packet
}

fn build_ps_payload() -> Bytes {
    let h264 = build_h264_keyframe();
    let pes_length = (h264.len() + 3) as u16;

    let mut buf = Vec::with_capacity(6 + pes_length as usize);
    buf.extend_from_slice(&[0x00, 0x00, 0x01, 0xE0]);
    buf.extend_from_slice(&pes_length.to_be_bytes());
    buf.extend_from_slice(&[0x80, 0x00, 0x00]);
    buf.extend_from_slice(&h264);

    Bytes::from(buf)
}

fn build_h264_keyframe() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[0, 0, 0, 1]);
    data.push(0x67);
    data.extend_from_slice(&[0x42, 0x00, 0x1f, 0xe9, 0x02, 0xc1, 0x2c, 0x80]);

    data.extend_from_slice(&[0, 0, 0, 1]);
    data.push(0x68);
    data.extend_from_slice(&[0xce, 0x3c, 0x80]);

    data.extend_from_slice(&[0, 0, 0, 1]);
    data.push(0x65);
    data.extend_from_slice(&vec![0x88; 120]);
    data
}
