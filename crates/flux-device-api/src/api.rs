use crate::{handlers, state::AppState};
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};

/// 创建 API 路由
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // 健康检查
        .route("/health", get(health_check))
        
        // 设备管理 API
        .route("/api/v1/devices", post(handlers::register_device))
        .route("/api/v1/devices", get(handlers::list_devices))
        .route("/api/v1/devices/:device_id", get(handlers::get_device))
        .route("/api/v1/devices/:device_id", put(handlers::update_device))
        .route("/api/v1/devices/:device_id", delete(handlers::delete_device))
        .route("/api/v1/devices/stats", get(handlers::get_stats))
        
        // 设备监控 API
        .route("/api/v1/devices/:device_id/heartbeat", post(handlers::heartbeat))
        .route("/api/v1/devices/:device_id/status", get(handlers::get_status))
        .route("/api/v1/devices/:device_id/online", get(handlers::is_online))
        .route("/api/v1/devices/:device_id/metrics", post(handlers::record_metric))
        .route("/api/v1/devices/:device_id/metrics", get(handlers::get_metrics))
        
        // 设备分组 API
        .route("/api/v1/groups", post(handlers::create_group))
        .route("/api/v1/groups", get(handlers::list_groups))
        .route("/api/v1/groups/:group_id", get(handlers::get_group))
        .route("/api/v1/groups/:group_id", delete(handlers::delete_group))
        .route("/api/v1/groups/:group_id/children", get(handlers::get_children))
        .route("/api/v1/groups/:group_id/devices", get(handlers::get_group_devices))
        .route("/api/v1/groups/:group_id/devices/:device_id", post(handlers::add_device_to_group))
        .route("/api/v1/groups/:group_id/devices/:device_id", delete(handlers::remove_device_from_group))
        
        // 添加中间件
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 健康检查
async fn health_check() -> &'static str {
    "OK"
}
