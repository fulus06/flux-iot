use crate::handlers::{
    cancel_command, create_scene, delete_scene, execute_batch_command, execute_scene,
    get_command_status, get_scene, list_device_commands, list_scenes, send_command, AppState,
    BatchAppState, SceneAppState,
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

pub fn create_scene_router(state: SceneAppState) -> Router {
    Router::new()
        // 场景管理
        .route("/api/v1/scenes", post(create_scene))
        .route("/api/v1/scenes", get(list_scenes))
        .route("/api/v1/scenes/:scene_id", get(get_scene))
        .route("/api/v1/scenes/:scene_id", delete(delete_scene))
        .route("/api/v1/scenes/:scene_id/execute", post(execute_scene))
        .with_state(state)
}

pub fn create_batch_router(state: BatchAppState) -> Router {
    Router::new()
        // 批量控制
        .route("/api/v1/batch/commands", post(execute_batch_command))
        .with_state(state)
}
