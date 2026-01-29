use clap::Parser;
use config::Config;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

// Import our core crates
use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[derive(Debug, Deserialize)]
struct AppConfig {
    server: ServerConfig,
    plugins: PluginConfig,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct PluginConfig {
    directory: String,
}

// Global Application State
struct AppState {
    event_bus: Arc<EventBus>,
    plugin_manager: Arc<PluginManager>,
    script_engine: Arc<ScriptEngine>,
    config: AppConfig,
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
    
    let state = Arc::new(AppState {
        event_bus: event_bus.clone(),
        plugin_manager: plugin_manager.clone(),
        script_engine: script_engine.clone(),
        config: app_config,
    });
    
    // 3. Start API Server (Axum)
    // TODO: move to api module
    let app = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "OK" }));

    let addr = format!("{}:{}", state.config.server.host, state.config.server.port);
    tracing::info!("Listening on {}", addr);
    
    axum::Server::bind(&addr.parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
