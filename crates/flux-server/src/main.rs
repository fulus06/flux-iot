use clap::Parser;
use config::Config;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

// Import our core crates
use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;

mod api;
mod worker;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub plugins: PluginConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct PluginConfig {
    pub directory: String,
}

// Global Application State
pub struct AppState {
    pub event_bus: Arc<EventBus>,
    pub plugin_manager: Arc<PluginManager>,
    pub script_engine: Arc<ScriptEngine>,
    pub config: AppConfig,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    tracing::info!("Starting FLUX IOT Server with config: {}", args.config);

    // 1. Load Config
    let settings = Config::builder()
        .add_source(config::File::with_name(&args.config))
        .build()?;
    
    let app_config: AppConfig = settings.try_deserialize()?;
    tracing::info!("Config loaded: {:?}", app_config);

    // 2. Initialize Core Components
    let event_bus = Arc::new(EventBus::new(1024));
    let plugin_manager = Arc::new(PluginManager::new()?);
    let script_engine = Arc::new(ScriptEngine::new());
    
    // Load Plugins
    // TODO: move to a proper loader service
    let plugin_dir = &app_config.plugins.directory;
    tracing::info!("Loading plugins from: {}", plugin_dir);
    
    if let Ok(entries) = std::fs::read_dir(plugin_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "wasm") {
                tracing::info!("Found plugin: {:?}", path);
                if let Ok(bytes) = std::fs::read(&path) {
                    let filename = path.file_stem().unwrap().to_string_lossy();
                    // Load the plugin
                    if let Err(e) = plugin_manager.load_plugin(&filename, &bytes) {
                        tracing::error!("Failed to load plugin {}: {:?}", filename, e);
                    } else {
                        tracing::info!("Successfully loaded plugin: {}", filename);
                    }
                }
            }
        }
    } else {
        tracing::warn!("Plugin directory not found: {}", plugin_dir);
    }

    let state = Arc::new(AppState {
        event_bus: event_bus.clone(),
        plugin_manager: plugin_manager.clone(),
        script_engine: script_engine.clone(),
        config: app_config,
    });
    
    
    // 3. Start API Server (Axum)
    let app = api::create_router(state.clone());
    
    // 4. Start Rule Worker
    let worker_state = state.clone();
    tokio::spawn(async move {
        worker::start_rule_worker(worker_state).await;
    });

    let addr = format!("{}:{}", state.config.server.host, state.config.server.port);
    tracing::info!("Listening on {}", addr);
    
    axum::Server::bind(&addr.parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
