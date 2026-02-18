use clap::Parser;
use flux_core::entity::{devices, prelude::*, rules};
use sea_orm::{Database, PaginatorTrait}; // SeaORM
use std::sync::Arc; // Entities

// Import our core crates
use flux_core::bus::EventBus;
use flux_plugin::PluginManager;
use flux_script::ScriptEngine;
use flux_video::gb28181::sip::SipServer;

// 使用 lib.rs 中定义的公共类型
use flux_server::{AppConfig, AppState};
use flux_server::config_provider::{AppConfigProvider, DbConfigProvider, FileConfigProvider};
use flux_server::config_manager::ConfigManager;

mod api;
mod auth;
mod metrics;
mod storage;
mod worker;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[arg(long, default_value = "file")]
    config_source: String,

    #[arg(long, default_value = "")]
    config_db_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,flux_server=debug");
    }
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    tracing::info!("Starting FLUX IOT Server with config: {}", args.config);

    // 1. Load Config (file for dev, database for test/prod)
    let config_source = std::env::var("FLUX_CONFIG_SOURCE").unwrap_or_else(|_| args.config_source);
    let config_db_url = match std::env::var("FLUX_CONFIG_DB_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => args.config_db_url,
    };

    let provider: Arc<dyn AppConfigProvider>;
    let app_config: AppConfig;
    let db;
    let config_db: Option<sea_orm::DatabaseConnection>;

    if config_source.eq_ignore_ascii_case("file") {
        provider = Arc::new(FileConfigProvider::new(args.config.clone()));
        app_config = provider.load().await?;
        tracing::info!("Config loaded from file: {:?}", app_config);

        tracing::info!("Connecting to database: {}", app_config.database.url);
        db = Database::connect(&app_config.database.url).await?;
        config_db = None;
    } else {
        let db_url = if !config_db_url.is_empty() {
            config_db_url
        } else if config_source.eq_ignore_ascii_case("sqlite")
            || config_source.eq_ignore_ascii_case("db")
            || config_source.eq_ignore_ascii_case("test")
        {
            "sqlite://flux.db?mode=rwc".to_string()
        } else if config_source.eq_ignore_ascii_case("postgres")
            || config_source.eq_ignore_ascii_case("prod")
        {
            std::env::var("DATABASE_URL")
                .map_err(|_e| anyhow::anyhow!("DATABASE_URL is required for postgres config_source"))?
        } else {
            return Err(anyhow::anyhow!("Unknown config_source: {}", config_source));
        };

        tracing::info!("Loading config from database: {}", db_url);
        let cfg_db = Database::connect(&db_url).await?;
        provider = Arc::new(DbConfigProvider::new(cfg_db.clone(), Some(args.config.clone())));
        app_config = provider.load().await?;
        tracing::info!("Config loaded from database: {:?}", app_config);

        config_db = Some(cfg_db.clone());

        if app_config.database.url == db_url {
            db = cfg_db;
        } else {
            tracing::info!("Connecting to database: {}", app_config.database.url);
            db = Database::connect(&app_config.database.url).await?;
        }
    }

    // 1.1 Start config manager (hot reload)
    let version = provider.version().await.unwrap_or(0);
    let config_manager = Arc::new(ConfigManager::new(provider, app_config.clone(), version));
    config_manager
        .clone()
        .start_polling(std::time::Duration::from_secs(2));
    let config_rx = config_manager.subscribe();

    // 2. Initialize Core Components
    let event_bus = Arc::new(EventBus::new(app_config.eventbus.capacity));
    let plugin_manager = Arc::new(PluginManager::new()?);
    let script_engine = Arc::new(ScriptEngine::new());

    // Create Tables (Simple Migration for MVP)
    use sea_orm::{ConnectionTrait, Schema};
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    let stmt = schema
        .create_table_from_entity(Rules)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&stmt)).await?;

    let stmt = schema
        .create_table_from_entity(Events)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&stmt)).await?;

    let stmt = schema
        .create_table_from_entity(Devices)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&stmt)).await?;
    tracing::info!("Database initialized and migrations applied.");

    // Seed Test Device
    let device_count = devices::Entity::find().count(&db).await?;
    if device_count == 0 {
        tracing::info!("Seeding test device...");
        let device = devices::ActiveModel {
            id: Set("test_device".to_owned()),
            token: Set(Some("password123".to_owned())),
            last_seen: Set(chrono::Utc::now().timestamp_millis()),
            ..Default::default()
        };
        device.insert(&db).await?;
    }

    // Seed Default Rule
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
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
            "#
            .to_owned()),
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
            if path.extension().is_some_and(|ext| ext == "wasm") {
                tracing::info!("Found plugin: {:?}", path);
                if let Ok(bytes) = std::fs::read(&path) {
                    let filename = match path.file_stem() {
                        Some(name) => name.to_string_lossy(),
                        None => {
                            tracing::warn!("Invalid plugin filename: {:?}", path);
                            continue;
                        }
                    };
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
        config_db,
        config: config_rx,
    });

    // 3. Initialize Metrics Exporter
    let metrics_addr = format!("{}:9090", app_config.server.host).parse()?;
    metrics::init_metrics(metrics_addr)?;

    // 设置初始指标值
    metrics::set_eventbus_capacity(app_config.eventbus.capacity);
    metrics::set_active_rules(script_engine.get_script_ids().len());
    metrics::set_database_connections(1);

    // 4. Start API Server (Axum)
    let app = api::create_router(state.clone());

    // 4.1 Start GB28181 SIP Server (optional)
    if app_config.gb28181.enabled {
        let sip_cfg = app_config.gb28181_sip_server_config();
        let sip = Arc::new(SipServer::new(sip_cfg).await?);

        let sip_task = sip.clone();
        tokio::spawn(async move {
            if let Err(e) = sip_task.start().await {
                tracing::error!("GB28181 SIP server stopped: {}", e);
            }
        });

        let mut cfg_rx = state.config.clone();
        let sip_to_update = sip.clone();
        tokio::spawn(async move {
            loop {
                if cfg_rx.changed().await.is_err() {
                    break;
                }

                let cfg = cfg_rx.borrow().clone();
                let new_sip_cfg = cfg.gb28181_sip_server_config();
                sip_to_update
                    .update_register_auth(
                        new_sip_cfg.auth_mode,
                        new_sip_cfg.auth_password,
                        new_sip_cfg.per_device_passwords,
                    )
                    .await;
            }
        });
    }

    // 5. Start Rule Worker
    let worker_state = state.clone();
    tokio::spawn(async move {
        worker::start_rule_worker(worker_state).await;
    });

    // 6. Start Storage Worker
    let storage_state = state.clone();
    tokio::spawn(async move {
        storage::start_storage_worker(storage_state).await;
    });

    // 7. Start MQTT Broker (Ntex)
    let mqtt_bus = state.event_bus.clone();
    let authenticator = Arc::new(auth::DbAuthenticator::new(state.db.clone()));
    flux_mqtt::start_broker(mqtt_bus, authenticator);

    let addr = format!("{}:{}", app_config.server.host, app_config.server.port);
    tracing::info!("Listening on {}", addr);

    axum::Server::bind(&addr.parse()?)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install signal handler: {}", e);
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("signal received, starting graceful shutdown");
}
