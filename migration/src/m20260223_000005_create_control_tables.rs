use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 设备指令表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("control"), DeviceCommands::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DeviceCommands::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DeviceCommands::DeviceId).string().not_null())
                    .col(ColumnDef::new(DeviceCommands::CommandType).string().not_null())
                    .col(ColumnDef::new(DeviceCommands::Params).json_binary())
                    .col(
                        ColumnDef::new(DeviceCommands::TimeoutSeconds)
                            .integer()
                            .not_null()
                            .default(30),
                    )
                    .col(
                        ColumnDef::new(DeviceCommands::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(DeviceCommands::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(DeviceCommands::SentAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(DeviceCommands::ExecutedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(DeviceCommands::CompletedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(DeviceCommands::Result).json_binary())
                    .col(ColumnDef::new(DeviceCommands::Error).text())
                    .to_owned(),
            )
            .await?;

        // 指令响应表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("control"), CommandResponses::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CommandResponses::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CommandResponses::CommandId).string().not_null())
                    .col(ColumnDef::new(CommandResponses::DeviceId).string().not_null())
                    .col(ColumnDef::new(CommandResponses::ResponseData).json_binary().not_null())
                    .col(
                        ColumnDef::new(CommandResponses::ReceivedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_command_responses_command_id")
                            .from((Alias::new("control"), CommandResponses::Table), CommandResponses::CommandId)
                            .to((Alias::new("control"), DeviceCommands::Table), DeviceCommands::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_device_commands_device_id")
                    .table((Alias::new("control"), DeviceCommands::Table))
                    .col(DeviceCommands::DeviceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_device_commands_status")
                    .table((Alias::new("control"), DeviceCommands::Table))
                    .col(DeviceCommands::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_device_commands_created_at")
                    .table((Alias::new("control"), DeviceCommands::Table))
                    .col(DeviceCommands::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_command_responses_command_id")
                    .table((Alias::new("control"), CommandResponses::Table))
                    .col(CommandResponses::CommandId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table((Alias::new("control"), CommandResponses::Table)).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table((Alias::new("control"), DeviceCommands::Table)).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum DeviceCommands {
    Table,
    Id,
    DeviceId,
    CommandType,
    Params,
    TimeoutSeconds,
    Status,
    CreatedAt,
    SentAt,
    ExecutedAt,
    CompletedAt,
    Result,
    Error,
}

#[derive(DeriveIden)]
enum CommandResponses {
    Table,
    Id,
    CommandId,
    DeviceId,
    ResponseData,
    ReceivedAt,
}
