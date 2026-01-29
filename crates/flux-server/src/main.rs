use clap::Parser;
use config::Config;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use sea_orm::{Database, DatabaseConnection, PaginatorTrait}; // SeaORM
use flux_core::entity::{prelude::*, rules, events, devices}; // Entities

// Import our core crates
use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;

mod api;
mod worker;
mod storage;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub plugins: PluginConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PluginConfig {
    pub directory: String,
}

// Global Application State
pub struct AppState {
    pub event_bus: Arc<EventBus>,
    pub plugin_manager: Arc<PluginManager>,
    pub script_engine: Arc<ScriptEngine>,
    pub db: DatabaseConnection,
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

    // Connect to Database
    tracing::info!("Connecting to database: {}", app_config.database.url);
    let db = Database::connect(&app_config.database.url).await?;
    
    // Create Tables (Simple Migration for MVP)
    use sea_orm::{Schema, ConnectionTrait};
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    
    let stmt = schema.create_table_from_entity(Rules).if_not_exists().to_owned();
    db.execute(backend.build(&stmt)).await?;
    
    let stmt = schema.create_table_from_entity(Events).if_not_exists().to_owned();
    db.execute(backend.build(&stmt)).await?;
    
    let stmt = schema.create_table_from_entity(Devices).if_not_exists().to_owned();
    db.execute(backend.build(&stmt)).await?;
    tracing::info!("Database initialized and migrations applied.");
    
    // Seed Default Rule
    use sea_orm::{EntityTrait, Set, ActiveModelTrait};
    let rule_count = rules::Entity::find().count(&db).await?;
    if rule_count == 0 {
        tracing::info!("Seeding default rule...");
        let rule = rules::ActiveModel {
            name: Set("default_temp_alert".to_owned()),
            script: Set(r#"
                if payload.value > 30.0 {
                    print("Alert: High Temperature detected! (From default)");
                    return true;
                }
                return false;
            "#.to_owned()),
            active: Set(true),
            created_at: Set(chrono::Utc::now().timestamp_millis()),
            ..Default::default() // Let DB handle ID if auto-increment (sqlite rowid)
        };
        rule.insert(&db).await?;
    }
    
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
        db: db.clone(),
        config: app_config,
    });
    
    
    // 3. Start API Server (Axum)
    let app = api::create_router(state.clone());
    
    // 4. Start Rule Worker
    let worker_state = state.clone();
    tokio::spawn(async move {
        worker::start_rule_worker(worker_state).await;
    });

    // 5. Start Storage Worker
    let storage_state = state.clone();
    tokio::spawn(async move {
        storage::start_storage_worker(storage_state).await;
    });

    let addr = format!("{}:{}", state.config.server.host, state.config.server.port);
    tracing::info!("Listening on {}", addr);
    
    axum::Server::bind(&addr.parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
