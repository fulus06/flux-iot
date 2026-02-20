use flux_shutdown::{
    ConnectionTracker, DatabaseResource, FileResource, ResourceManager, ShutdownCoordinator,
    SignalHandler, StateManager,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppState {
    request_count: u64,
    active_users: u64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    println!("=== FLUX IOT 优雅关闭系统示例 ===\n");

    // 1. 创建信号处理器
    println!("1. 创建信号处理器");
    let (signal_handler, mut shutdown_rx) = SignalHandler::new();
    println!("信号处理器已创建\n");

    // 2. 创建连接跟踪器
    println!("2. 创建连接跟踪器（最大排空时间: 30秒）");
    let connection_tracker = ConnectionTracker::new(Duration::from_secs(30));
    println!("连接跟踪器已创建\n");

    // 3. 创建资源管理器
    println!("3. 创建资源管理器");
    let mut resource_manager = ResourceManager::new();

    // 注册数据库资源
    resource_manager.register(Arc::new(DatabaseResource::new("postgres".to_string())));

    // 注册文件资源
    resource_manager.register(Arc::new(FileResource::new(
        "log_file".to_string(),
        "/var/log/flux-iot.log".to_string(),
    )));

    println!("已注册 {} 个资源\n", resource_manager.count());

    // 4. 创建状态管理器
    println!("4. 创建状态管理器");
    let initial_state = AppState {
        request_count: 0,
        active_users: 0,
    };
    let state_manager = StateManager::new(initial_state, "/tmp/flux-iot-state.json");
    println!("状态管理器已创建\n");

    // 5. 模拟服务运行
    println!("5. 模拟服务运行");
    println!("提示: 按 Ctrl+C 触发优雅关闭\n");

    // 模拟一些活跃连接
    let conn1 = connection_tracker.acquire();
    let conn2 = connection_tracker.acquire();
    let conn3 = connection_tracker.acquire();

    println!("当前活跃连接数: {}\n", connection_tracker.active_count());

    // 模拟更新状态
    {
        let mut state = state_manager.get_mut().await;
        state.request_count = 1000;
        state.active_users = 50;
    }

    // 创建关闭协调器
    let coordinator = ShutdownCoordinator::builder()
        .with_signal_handler(signal_handler)
        .with_connection_tracker(connection_tracker)
        .with_resource_manager(resource_manager)
        .with_shutdown_timeout(Duration::from_secs(60))
        .with_drain_timeout(Duration::from_secs(30))
        .build();

    // 手动触发关闭（模拟 Ctrl+C）
    println!("6. 手动触发关闭信号（模拟 Ctrl+C）");
    coordinator.signal_handler().trigger_shutdown();

    // 在后台释放连接
    tokio::spawn(async move {
        println!("  [后台任务] 等待 1 秒后释放连接...");
        sleep(Duration::from_secs(1)).await;
        drop(conn1);
        println!("  [后台任务] 连接 1 已释放");

        sleep(Duration::from_millis(500)).await;
        drop(conn2);
        println!("  [后台任务] 连接 2 已释放");

        sleep(Duration::from_millis(500)).await;
        drop(conn3);
        println!("  [后台任务] 连接 3 已释放");
    });

    // 保存状态
    println!("\n7. 保存应用状态");
    state_manager.save_checkpoint().await.unwrap();
    println!("状态已保存到检查点\n");

    // 运行关闭流程
    println!("8. 开始优雅关闭流程\n");
    let phase = coordinator.run().await;

    println!("\n=== 关闭完成 ===");
    println!("最终阶段: {:?}", phase);

    // 验证状态可以恢复
    println!("\n9. 验证状态恢复");
    let recovered_state = StateManager::new(
        AppState {
            request_count: 0,
            active_users: 0,
        },
        "/tmp/flux-iot-state.json",
    );
    recovered_state.load_checkpoint().await.unwrap();

    let state = recovered_state.get().await;
    println!("恢复的状态:");
    println!("  请求数: {}", state.request_count);
    println!("  活跃用户: {}", state.active_users);

    println!("\n=== 示例完成 ===");
}
