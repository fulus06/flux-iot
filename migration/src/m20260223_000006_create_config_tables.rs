use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 应用配置表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("public"), AppConfig::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AppConfig::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AppConfig::Content).text().not_null())
                    .col(
                        ColumnDef::new(AppConfig::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 配置审计表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("public"), AppConfigAudit::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AppConfigAudit::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AppConfigAudit::PrevUpdatedAt).big_integer())
                    .col(ColumnDef::new(AppConfigAudit::NewUpdatedAt).big_integer().not_null())
                    .col(ColumnDef::new(AppConfigAudit::PrevHash).text())
                    .col(ColumnDef::new(AppConfigAudit::NewHash).text().not_null())
                    .col(ColumnDef::new(AppConfigAudit::UserAgent).text())
                    .col(ColumnDef::new(AppConfigAudit::ForwardedFor).text())
                    .col(
                        ColumnDef::new(AppConfigAudit::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 规则表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("public"), Rules::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Rules::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Rules::Name).string().not_null())
                    .col(ColumnDef::new(Rules::Description).text())
                    .col(ColumnDef::new(Rules::TriggerType).string().not_null())
                    .col(ColumnDef::new(Rules::TriggerConfig).json_binary())
                    .col(ColumnDef::new(Rules::Script).text().not_null())
                    .col(
                        ColumnDef::new(Rules::Priority)
                            .integer()
                            .not_null()
                            .default(50),
                    )
                    .col(
                        ColumnDef::new(Rules::Active)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Rules::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 事件表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("public"), Events::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Events::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Events::EventType).string().not_null())
                    .col(ColumnDef::new(Events::Source).string().not_null())
                    .col(ColumnDef::new(Events::Payload).json_binary())
                    .col(
                        ColumnDef::new(Events::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_app_config_audit_created_at")
                    .table((Alias::new("public"), AppConfigAudit::Table))
                    .col(AppConfigAudit::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_rules_active")
                    .table((Alias::new("public"), Rules::Table))
                    .col(Rules::Active)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_events_timestamp")
                    .table((Alias::new("public"), Events::Table))
                    .col(Events::Timestamp)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_events_event_type")
                    .table((Alias::new("public"), Events::Table))
                    .col(Events::EventType)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table((Alias::new("public"), Events::Table)).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table((Alias::new("public"), Rules::Table)).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table((Alias::new("public"), AppConfigAudit::Table)).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table((Alias::new("public"), AppConfig::Table)).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AppConfig {
    Table,
    Id,
    Content,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum AppConfigAudit {
    Table,
    Id,
    PrevUpdatedAt,
    NewUpdatedAt,
    PrevHash,
    NewHash,
    UserAgent,
    ForwardedFor,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Rules {
    Table,
    Id,
    Name,
    Description,
    TriggerType,
    TriggerConfig,
    Script,
    Priority,
    Active,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Events {
    Table,
    Id,
    EventType,
    Source,
    Payload,
    Timestamp,
}
