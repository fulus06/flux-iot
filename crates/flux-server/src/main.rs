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
use flux_server::config::Gb28181Backend;
use flux_server::gb28181_backend::{EmbeddedBackend, Gb28181BackendRef, RemoteBackend};
use flux_storage::{DiskType, PoolConfig, StorageManager};
use std::path::PathBuf;
use flux_rule::RuleServices;
use async_trait::async_trait;
use flux_control::{CommandExecutor, CommandType, DeviceCommand};
use flux_control::channel::MqttCommandChannel;

mod api;
mod auth;
mod metrics;
mod storage;
mod worker;

struct ServerRuleServices {
    event_bus: Arc<EventBus>,
    db: sea_orm::DatabaseConnection,
    command_executor: Arc<CommandExecutor>,
    webhook_url: Option<String>,
    http_client: reqwest::Client,
}

impl ServerRuleServices {
    fn publish_json(&self, topic: String, payload: serde_json::Value) {
        let msg = flux_types::message::Message::new(topic, payload);
        if let Err(e) = self.event_bus.publish(msg) {
            tracing::warn!(error = %e, "Failed to publish rule action event");
        }
    }

    async fn post_webhook(&self, payload: serde_json::Value) -> anyhow::Result<()> {
        let Some(url) = self.webhook_url.as_ref() else {
            return Ok(());
        };

        let resp = self
            .http_client
            .post(url)
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "Webhook returned non-success status: {}",
                resp.status()
            ));
        }

        Ok(())
    }

    fn parse_time_range_to_ms(time_range: &str) -> Option<i64> {
        let trimmed = time_range.trim();
        if trimmed.is_empty() {
            return None;
        }

        let (num_part, unit) = trimmed.split_at(trimmed.len().saturating_sub(1));
        let value: i64 = match num_part.parse() {
            Ok(v) => v,
            Err(_) => return None,
        };

        let mult = match unit {
            "s" | "S" => 1_000,
            "m" | "M" => 60_000,
            "h" | "H" => 3_600_000,
            "d" | "D" => 86_400_000,
            _ => return None,
        };

        Some(value.saturating_mul(mult))
    }
}

#[async_trait]
impl RuleServices for ServerRuleServices {
    async fn control_device(
        &self,
        device_id: &str,
        command: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()> {
        let cmd = DeviceCommand::new(
            device_id.to_string(),
            CommandType::Custom {
                name: command.to_string(),
                params: params.clone(),
            },
        );

        let _command_id = self.command_executor.submit(cmd.clone()).await?;

        let exec = self.command_executor.clone();
        tokio::spawn(async move {
            if let Err(e) = exec.execute(cmd).await {
                tracing::warn!(error = %e, "Rule control_device execution failed");
            }
        });

        self.publish_json(
            format!("rule/control/{}", device_id),
            serde_json::json!({"device_id": device_id, "command": command, "params": params}),
        );
        Ok(())
    }

    async fn read_device(&self, device_id: &str, metric: &str) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({"device_id": device_id, "metric": metric, "value": null}))
    }

    async fn update_device_status(&self, device_id: &str, status: &str) -> anyhow::Result<()> {
        self.publish_json(
            format!("rule/device/{}/status", device_id),
            serde_json::json!({"device_id": device_id, "status": status}),
        );
        Ok(())
    }

    async fn send_notification(&self, channel: &str, title: &str, message: &str) -> anyhow::Result<()> {
        let webhook_payload = serde_json::json!({
            "type": "notification",
            "channel": channel,
            "title": title,
            "message": message,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        });

        if let Err(e) = self.post_webhook(webhook_payload).await {
            tracing::warn!(error = %e, "Webhook notification failed");
        }

        self.publish_json(
            format!("rule/notify/{}", channel),
            serde_json::json!({"channel": channel, "title": title, "message": message}),
        );
        Ok(())
    }

    async fn send_email(&self, params: serde_json::Value) -> anyhow::Result<()> {
        let webhook_payload = serde_json::json!({
            "type": "email",
            "params": params,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        });

        if let Err(e) = self.post_webhook(webhook_payload).await {
            tracing::warn!(error = %e, "Webhook email failed");
        }

        self.publish_json("rule/notify/email".to_string(), params);
        Ok(())
    }

    async fn send_sms(&self, phone: &str, message: &str) -> anyhow::Result<()> {
        let webhook_payload = serde_json::json!({
            "type": "sms",
            "phone": phone,
            "message": message,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        });

        if let Err(e) = self.post_webhook(webhook_payload).await {
            tracing::warn!(error = %e, "Webhook sms failed");
        }

        self.publish_json(
            "rule/notify/sms".to_string(),
            serde_json::json!({"phone": phone, "message": message}),
        );
        Ok(())
    }

    async fn send_push(&self, user_id: &str, title: &str, message: &str) -> anyhow::Result<()> {
        let webhook_payload = serde_json::json!({
            "type": "push",
            "user_id": user_id,
            "title": title,
            "message": message,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        });

        if let Err(e) = self.post_webhook(webhook_payload).await {
            tracing::warn!(error = %e, "Webhook push failed");
        }

        self.publish_json(
            "rule/notify/push".to_string(),
            serde_json::json!({"user_id": user_id, "title": title, "message": message}),
        );
        Ok(())
    }

    async fn query_metrics(&self, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        use flux_core::entity::events;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let topic = params
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if topic.is_empty() {
            return Ok(serde_json::json!({}));
        }

        let window_ms = params
            .get("window_ms")
            .and_then(|v| v.as_i64())
            .or_else(|| {
                params
                    .get("time_range")
                    .and_then(|v| v.as_str())
                    .and_then(Self::parse_time_range_to_ms)
            })
            .unwrap_or(3_600_000);

        let cutoff = chrono::Utc::now().timestamp_millis().saturating_sub(window_ms);

        let rows = events::Entity::find()
            .filter(events::Column::Topic.eq(topic.to_string()))
            .filter(events::Column::Timestamp.gte(cutoff))
            .all(&self.db)
            .await?;

        let field = params
            .get("field")
            .and_then(|v| v.as_str())
            .unwrap_or("value");

        let mut vals: Vec<f64> = Vec::new();
        for row in rows {
            let payload: serde_json::Value = row.payload;
            if let Some(v) = payload.get(field).and_then(|vv| vv.as_f64()) {
                vals.push(v);
            }
        }

        if vals.is_empty() {
            return Ok(serde_json::json!({"total": 0.0, "average": 0.0, "peak": 0.0}));
        }

        let total: f64 = vals.iter().sum();
        let average = total / (vals.len() as f64);
        let peak = vals
            .into_iter()
            .fold(f64::MIN, |acc, x| if x > acc { x } else { acc });

        Ok(serde_json::json!({"total": total, "average": average, "peak": peak}))
    }

    async fn count_events(&self, event_type: &str, time_range: &str) -> anyhow::Result<i64> {
        use flux_core::entity::events;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let window_ms = Self::parse_time_range_to_ms(time_range).unwrap_or(3_600_000);
        let cutoff = chrono::Utc::now().timestamp_millis().saturating_sub(window_ms);

        let count = events::Entity::find()
            .filter(events::Column::Topic.eq(event_type.to_string()))
            .filter(events::Column::Timestamp.gte(cutoff))
            .count(&self.db)
            .await?;

        Ok(count as i64)
    }

    async fn record_event(&self, event_type: &str, data: serde_json::Value) -> anyhow::Result<()> {
        use flux_core::entity::events;
        use sea_orm::{ActiveModelTrait, Set};

        let model = events::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            topic: Set(event_type.to_string()),
            payload: Set(data),
            timestamp: Set(chrono::Utc::now().timestamp_millis()),
        };

        let _ = model.insert(&self.db).await?;
        Ok(())
    }

    async fn create_ticket(&self, params: serde_json::Value) -> anyhow::Result<()> {
        self.publish_json("rule/ticket/create".to_string(), params);
        Ok(())
    }

    async fn update_ticket(&self, ticket_id: &str, params: serde_json::Value) -> anyhow::Result<()> {
        self.publish_json(
            format!("rule/ticket/{}/update", ticket_id),
            params,
        );
        Ok(())
    }

    async fn close_ticket(&self, ticket_id: &str) -> anyhow::Result<()> {
        self.publish_json(
            format!("rule/ticket/{}/close", ticket_id),
            serde_json::json!({"ticket_id": ticket_id}),
        );
        Ok(())
    }
}

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
            "postgresql://flux:flux@localhost/flux_iot".to_string()
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

    let webhook_url = app_config.rule.webhook_url.clone();
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let mqtt_channel = MqttCommandChannel::new("127.0.0.1", 1883, "flux_rule_executor").await?;
    let command_channel: Arc<dyn flux_control::CommandChannel> = Arc::new(mqtt_channel);
    let command_executor = Arc::new(CommandExecutor::new(command_channel));

    let services = Arc::new(ServerRuleServices {
        event_bus: event_bus.clone(),
        db: db.clone(),
        command_executor,
        webhook_url,
        http_client,
    });

    let mut script_engine_inner = ScriptEngine::new();
    flux_rule::functions::register_builtin_functions_with_services(&mut script_engine_inner, services);
    let script_engine = Arc::new(script_engine_inner);

    // 2.1 Initialize StorageManager (multi-pool from ./config)
    let storage_manager = Arc::new(StorageManager::new());
    let storage_cfg_loader = flux_config::ConfigLoader::new("./config");
    let pool_configs = match storage_cfg_loader.load_storage_pools("server") {
        Ok(Some(pools)) => pools,
        Ok(None) => vec![PoolConfig {
            name: "default".to_string(),
            path: PathBuf::from("./data"),
            disk_type: DiskType::Unknown,
            priority: 1,
            max_usage_percent: 95.0,
        }],
        Err(e) => {
            tracing::warn!(target: "flux_server", "Failed to load storage pools config, fallback to ./data: {}", e);
            vec![PoolConfig {
                name: "default".to_string(),
                path: PathBuf::from("./data"),
                disk_type: DiskType::Unknown,
                priority: 1,
                max_usage_percent: 95.0,
            }]
        }
    };
    if let Err(e) = storage_manager.initialize(pool_configs).await {
        tracing::warn!(target: "flux_server", "StorageManager initialize failed: {}", e);
    }
    let storage_health_handle = storage_manager.clone().start_health_check_task_handle();

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

    // Load Plugins using PluginLoader service
    let plugin_loader = flux_server::plugin_loader::PluginLoader::new(
        &app_config.plugins.directory,
        plugin_manager.clone(),
    );
    
    match plugin_loader.load_all().await {
        Ok(result) => {
            tracing::info!(
                total = result.total,
                loaded = result.loaded,
                failed = result.failed.len(),
                success_rate = format!("{:.1}%", result.success_rate() * 100.0),
                "Plugin loading completed"
            );
            
            // 记录加载失败的插件
            for error in &result.failed {
                tracing::error!(
                    path = %error.path.display(),
                    error = %error.error,
                    "Plugin load failed"
                );
            }
            
            // 更新 metrics
            flux_server::metrics::set_loaded_plugins(result.loaded);
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to load plugins");
        }
    }

    // Prepare optional GB28181 backend (embedded or remote)
    let (gb28181_sip, gb28181_backend): (Option<Arc<SipServer>>, Option<Gb28181BackendRef>) =
        if !app_config.gb28181.enabled {
            (None, None)
        } else {
            match app_config.gb28181.backend {
                Gb28181Backend::Embedded => {
                    let sip_cfg = app_config.gb28181_sip_server_config();
                    let sip = Arc::new(SipServer::new(sip_cfg).await?);
                    let backend: Gb28181BackendRef = Arc::new(EmbeddedBackend::new(sip.clone()));
                    (Some(sip), Some(backend))
                }
                Gb28181Backend::Remote => {
                    let base_url = app_config
                        .gb28181
                        .remote
                        .base_url
                        .clone()
                        .ok_or_else(|| {
                            anyhow::anyhow!("gb28181.remote.base_url is required when backend=remote")
                        })?;
                    let backend: Gb28181BackendRef = Arc::new(RemoteBackend::new(base_url));
                    (None, Some(backend))
                }
            }
        };

    let state = Arc::new(AppState {
        event_bus: event_bus.clone(),
        plugin_manager: plugin_manager.clone(),
        script_engine: script_engine.clone(),
        storage_manager: storage_manager.clone(),
        db: db.clone(),
        config_db,
        config: config_rx,
        gb28181_sip: gb28181_sip.clone(),
        gb28181_backend: gb28181_backend.clone(),
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

    // 4.1 Start GB28181 SIP Server (embedded only)
    if let Some(sip) = gb28181_sip {
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

    // 6.1 Start Storage Metrics Worker
    let storage_metrics_state = state.clone();
    let storage_metrics_handle = tokio::spawn(async move {
        storage::start_storage_metrics_worker(storage_metrics_state).await;
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

    storage_health_handle.shutdown().await;
    storage_metrics_handle.abort();

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
