// 导出配置模块供测试使用
pub mod api;
pub mod config;
pub mod config_provider;
pub mod config_manager;
pub mod metrics;
pub mod plugin_loader;
pub mod storage;
pub mod gb28181_backend;

use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::watch;
use flux_video::gb28181::sip::SipServer;
use crate::gb28181_backend::Gb28181BackendRef;
use flux_storage::StorageManager;

// 重新导出配置类型
pub use config::AppConfig;

// 定义 AppState（供 main.rs 和测试使用）
pub struct AppState {
    pub event_bus: Arc<EventBus>,
    pub plugin_manager: Arc<PluginManager>,
    pub script_engine: Arc<ScriptEngine>,
    pub storage_manager: Arc<StorageManager>,
    pub db: DatabaseConnection,
    pub config_db: Option<DatabaseConnection>,
    pub config: watch::Receiver<AppConfig>,
    pub gb28181_sip: Option<Arc<SipServer>>,
    pub gb28181_backend: Option<Gb28181BackendRef>,
}
