use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建 RTMPD Schema
        manager
            .get_connection()
            .execute_unprepared("CREATE SCHEMA IF NOT EXISTS rtmpd;")
            .await?;

        // 创建 MQTT Schema
        manager
            .get_connection()
            .execute_unprepared("CREATE SCHEMA IF NOT EXISTS mqtt;")
            .await?;

        // 创建 Device Schema
        manager
            .get_connection()
            .execute_unprepared("CREATE SCHEMA IF NOT EXISTS device;")
            .await?;

        // 创建 Control Schema
        manager
            .get_connection()
            .execute_unprepared("CREATE SCHEMA IF NOT EXISTS control;")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP SCHEMA IF EXISTS control CASCADE;")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP SCHEMA IF EXISTS device CASCADE;")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP SCHEMA IF EXISTS mqtt CASCADE;")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP SCHEMA IF EXISTS rtmpd CASCADE;")
            .await?;

        Ok(())
    }
}
