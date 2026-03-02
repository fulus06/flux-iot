use crate::handlers::{
    cancel_command, execute_batch_command,
    get_command_status, list_device_commands, send_command, AppState,
    BatchAppState,
};
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // 指令管理
        .route("/api/v1/devices/:device_id/commands", post(send_command))
        .route(
            "/api/v1/devices/:device_id/commands",
            get(list_device_commands),
        )
        .route("/api/v1/commands/:command_id", get(get_command_status))
        .route("/api/v1/commands/:command_id", delete(cancel_command))
        .with_state(state)
}


pub fn create_batch_router(state: BatchAppState) -> Router {
    Router::new()
        // 批量控制
        .route("/api/v1/batch/commands", post(execute_batch_command))
        .with_state(state)
}
