use crate::error::ApiError;
use axum::{
    extract::{Path, State},
    Json,
};
use flux_control::{Scene, SceneEngine, TriggerManager};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 场景 API 状态
#[derive(Clone)]
pub struct SceneAppState {
    pub scene_engine: Arc<SceneEngine>,
    pub trigger_manager: Arc<TriggerManager>,
}

/// 创建场景请求
#[derive(Debug, Deserialize)]
pub struct CreateSceneRequest {
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<serde_json::Value>,
    pub condition_script: Option<String>,
    pub action_script: String,
}

/// 场景响应
#[derive(Debug, Serialize)]
pub struct SceneResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: String,
}

/// 创建场景
pub async fn create_scene(
    State(state): State<SceneAppState>,
    Json(req): Json<CreateSceneRequest>,
) -> Result<Json<SceneResponse>, ApiError> {
    let mut scene = Scene::new(req.name.clone(), req.action_script);
    
    if let Some(desc) = req.description {
        scene = scene.with_description(desc);
    }
    
    if let Some(condition) = req.condition_script {
        scene = scene.with_condition(condition);
    }
    
    // 编译场景脚本
    state.scene_engine.compile_scene(&scene).await?;
    
    // 注册到触发器管理器
    state.trigger_manager.register_scene(scene.clone()).await;
    
    Ok(Json(SceneResponse {
        id: scene.id.clone(),
        name: scene.name.clone(),
        description: scene.description.clone(),
        enabled: scene.enabled,
        created_at: scene.created_at.to_rfc3339(),
    }))
}

/// 获取场景
pub async fn get_scene(
    State(state): State<SceneAppState>,
    Path(scene_id): Path<String>,
) -> Result<Json<Scene>, ApiError> {
    let scene = state
        .trigger_manager
        .get_scene(&scene_id)
        .await
        .ok_or_else(|| ApiError::NotFound(format!("Scene {} not found", scene_id)))?;
    
    Ok(Json(scene))
}

/// 列出所有场景
pub async fn list_scenes(
    State(state): State<SceneAppState>,
) -> Result<Json<Vec<SceneResponse>>, ApiError> {
    let scenes = state.trigger_manager.list_scenes().await;
    
    let responses: Vec<SceneResponse> = scenes
        .into_iter()
        .map(|scene| SceneResponse {
            id: scene.id,
            name: scene.name,
            description: scene.description,
            enabled: scene.enabled,
            created_at: scene.created_at.to_rfc3339(),
        })
        .collect();
    
    Ok(Json(responses))
}

/// 执行场景
pub async fn execute_scene(
    State(state): State<SceneAppState>,
    Path(scene_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scene = state
        .trigger_manager
        .trigger_scene(&scene_id)
        .await
        .ok_or_else(|| ApiError::NotFound(format!("Scene {} not found", scene_id)))?;
    
    let execution = state.scene_engine.execute_scene(&scene).await?;
    
    Ok(Json(serde_json::json!({
        "scene_id": execution.scene_id,
        "executed_at": execution.executed_at.to_rfc3339(),
        "success": execution.success,
        "error": execution.error,
        "duration_ms": execution.duration_ms,
    })))
}

/// 删除场景
pub async fn delete_scene(
    State(state): State<SceneAppState>,
    Path(scene_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.trigger_manager.unregister_scene(&scene_id).await;
    
    Ok(Json(serde_json::json!({
        "message": "Scene deleted successfully",
        "scene_id": scene_id,
    })))
}
