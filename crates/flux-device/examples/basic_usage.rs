/// flux-device 基本使用示例
/// 
/// 演示设备管理的基本操作流程

use flux_device::{
    Device, DeviceFilter, DeviceGroup, DeviceManager, DeviceStatus, DeviceType, Protocol,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== FLUX IOT 设备管理示例 ===\n");

    // 1. 创建数据库连接（示例使用默认连接）
    // 实际使用时应该连接到真实的 PostgreSQL 数据库
    // let db = Database::connect("postgres://user:pass@localhost/flux_iot").await?;
    let db = Arc::new(DatabaseConnection::default());
    println!("✓ 数据库连接已建立");

    // 2. 创建设备管理器
    // 参数：心跳间隔30秒，超时60秒
    let manager = DeviceManager::new(db, 30, 60);
    manager.start().await;
    println!("✓ 设备管理器已启动\n");

    // 3. 创建设备分组
    println!("--- 创建设备分组 ---");
    let floor1 = DeviceGroup::new("一楼".to_string(), None);
    let floor1_id = floor1.id.clone();
    manager.create_group(floor1).await?;
    println!("✓ 创建分组: 一楼 ({})", floor1_id);

    let room101 = DeviceGroup::new("101房间".to_string(), Some(floor1_id.clone()));
    let room101_id = room101.id.clone();
    manager.create_group(room101).await?;
    println!("✓ 创建子分组: 101房间 ({})\n", room101_id);

    // 4. 注册设备
    println!("--- 注册设备 ---");
    
    // 温度传感器
    let mut temp_sensor = Device::new(
        "温度传感器-01".to_string(),
        DeviceType::Sensor,
        Protocol::MQTT,
    );
    temp_sensor.add_tag("temperature".to_string());
    temp_sensor.add_tag("indoor".to_string());
    temp_sensor.set_metadata("model".to_string(), "DHT22".to_string());
    temp_sensor.set_metadata("manufacturer".to_string(), "ACME".to_string());
    
    let temp_id = temp_sensor.id.clone();
    manager.register_device(temp_sensor).await?;
    println!("✓ 注册温度传感器: {}", temp_id);

    // 湿度传感器
    let mut humidity_sensor = Device::new(
        "湿度传感器-01".to_string(),
        DeviceType::Sensor,
        Protocol::MQTT,
    );
    humidity_sensor.add_tag("humidity".to_string());
    humidity_sensor.add_tag("indoor".to_string());
    
    let humidity_id = humidity_sensor.id.clone();
    manager.register_device(humidity_sensor).await?;
    println!("✓ 注册湿度传感器: {}", humidity_id);

    // 摄像头
    let camera = Device::new(
        "监控摄像头-01".to_string(),
        DeviceType::Camera,
        Protocol::RTSP,
    );
    let camera_id = camera.id.clone();
    manager.register_device(camera).await?;
    println!("✓ 注册监控摄像头: {}\n", camera_id);

    // 5. 添加设备到分组
    println!("--- 添加设备到分组 ---");
    manager.add_to_group(&room101_id, &temp_id).await?;
    println!("✓ 温度传感器 -> 101房间");
    
    manager.add_to_group(&room101_id, &humidity_id).await?;
    println!("✓ 湿度传感器 -> 101房间");
    
    manager.add_to_group(&room101_id, &camera_id).await?;
    println!("✓ 监控摄像头 -> 101房间\n");

    // 6. 查询分组下的设备
    println!("--- 查询分组设备 ---");
    let devices = manager.get_group_devices(&room101_id).await?;
    println!("101房间共有 {} 个设备:", devices.len());
    for device in &devices {
        println!("  - {} ({})", device.name, device.device_type.as_str());
    }
    println!();

    // 7. 设备心跳和状态管理
    println!("--- 设备心跳和状态 ---");
    
    // 温度传感器上线
    manager.heartbeat(&temp_id).await?;
    let status = manager.get_status(&temp_id).await?;
    println!("✓ 温度传感器心跳 -> 状态: {:?}", status);

    // 湿度传感器上线
    manager.heartbeat(&humidity_id).await?;
    println!("✓ 湿度传感器心跳 -> 状态: Online");

    // 摄像头设置为维护状态
    manager.set_status(&camera_id, DeviceStatus::Maintenance).await?;
    println!("✓ 摄像头设置为维护状态\n");

    // 8. 记录设备指标
    println!("--- 记录设备指标 ---");
    manager.record_metric(
        &temp_id,
        "temperature".to_string(),
        25.5,
        Some("°C".to_string()),
    ).await?;
    println!("✓ 温度: 25.5°C");

    manager.record_metric(
        &humidity_id,
        "humidity".to_string(),
        60.0,
        Some("%".to_string()),
    ).await?;
    println!("✓ 湿度: 60.0%\n");

    // 9. 设备过滤查询
    println!("--- 设备过滤查询 ---");
    
    // 查询所有传感器
    let filter = DeviceFilter {
        device_type: Some(DeviceType::Sensor),
        ..Default::default()
    };
    let sensors = manager.list_devices(filter).await?;
    println!("传感器数量: {}", sensors.len());

    // 查询在线设备
    let filter = DeviceFilter {
        status: Some(DeviceStatus::Online),
        ..Default::default()
    };
    let online_devices = manager.list_devices(filter).await?;
    println!("在线设备数量: {}", online_devices.len());

    // 按标签查询
    let filter = DeviceFilter {
        tags: Some(vec!["indoor".to_string()]),
        ..Default::default()
    };
    let indoor_devices = manager.list_devices(filter).await?;
    println!("室内设备数量: {}\n", indoor_devices.len());

    // 10. 统计信息
    println!("--- 统计信息 ---");
    let total = manager.count_devices(DeviceFilter::default()).await?;
    let online = manager.online_count().await?;
    let offline = manager.offline_count().await?;
    
    println!("总设备数: {}", total);
    println!("在线设备: {}", online);
    println!("离线设备: {}", offline);
    println!();

    // 11. 更新设备信息
    println!("--- 更新设备信息 ---");
    let mut device = manager.get_device(&temp_id).await?.unwrap();
    device.name = "温度传感器-01-已更新".to_string();
    device.add_tag("calibrated".to_string());
    manager.update_device(&temp_id, device).await?;
    println!("✓ 温度传感器信息已更新\n");

    // 12. 模拟心跳超时
    println!("--- 模拟设备运行 ---");
    println!("设备持续发送心跳...");
    for i in 1..=3 {
        sleep(Duration::from_secs(2)).await;
        manager.heartbeat(&temp_id).await?;
        manager.heartbeat(&humidity_id).await?;
        println!("  心跳 #{}", i);
    }
    println!();

    // 13. 清理（可选）
    println!("--- 清理示例数据 ---");
    // 在实际应用中，通常不需要删除设备
    // 这里仅作演示
    println!("提示: 在实际应用中，设备数据会持久化保存");
    println!();

    // 14. 停止管理器
    manager.stop().await;
    println!("✓ 设备管理器已停止");
    
    println!("\n=== 示例完成 ===");
    
    Ok(())
}
