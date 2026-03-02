pub use sea_orm_migration::prelude::*;

mod m20260223_000001_create_schemas;
mod m20260223_000002_create_users_table;
mod m20260223_000003_create_devices_tables;
mod m20260223_000004_create_mqtt_tables;
mod m20260223_000005_create_control_tables;
mod m20260223_000006_create_config_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260223_000001_create_schemas::Migration),
            Box::new(m20260223_000002_create_users_table::Migration),
            Box::new(m20260223_000003_create_devices_tables::Migration),
            Box::new(m20260223_000004_create_mqtt_tables::Migration),
            Box::new(m20260223_000005_create_control_tables::Migration),
            Box::new(m20260223_000006_create_config_tables::Migration),
        ]
    }
}
