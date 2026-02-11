// 导出公共模块供测试使用
pub mod api;
pub mod config;

use std::sync::Arc;
use sea_orm::DatabaseConnection;
use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;

// 重新导出配置类型
pub use config::AppConfig;

// 全局应用状态
pub struct AppState {
    pub event_bus: Arc<EventBus>,
    pub plugin_manager: Arc<PluginManager>,
    pub script_engine: Arc<ScriptEngine>,
    pub db: DatabaseConnection,
    pub config: AppConfig,
}
