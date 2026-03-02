use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 设备表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("device"), Devices::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Devices::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Devices::Name).string().not_null())
                    .col(ColumnDef::new(Devices::DeviceType).string().not_null())
                    .col(ColumnDef::new(Devices::Protocol).string().not_null())
                    .col(ColumnDef::new(Devices::Status).string().not_null())
                    .col(ColumnDef::new(Devices::Metadata).json_binary())
                    .col(
                        ColumnDef::new(Devices::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Devices::UpdatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Devices::LastSeenAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        // 设备指标表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("device"), DeviceMetrics::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DeviceMetrics::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DeviceMetrics::DeviceId).string().not_null())
                    .col(ColumnDef::new(DeviceMetrics::MetricName).string().not_null())
                    .col(ColumnDef::new(DeviceMetrics::Value).double().not_null())
                    .col(
                        ColumnDef::new(DeviceMetrics::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_device_metrics_device_id")
                            .from((Alias::new("device"), DeviceMetrics::Table), DeviceMetrics::DeviceId)
                            .to((Alias::new("device"), Devices::Table), Devices::Id)
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
                    .name("idx_devices_status")
                    .table((Alias::new("device"), Devices::Table))
                    .col(Devices::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_devices_type")
                    .table((Alias::new("device"), Devices::Table))
                    .col(Devices::DeviceType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_device_metrics_device_id")
                    .table((Alias::new("device"), DeviceMetrics::Table))
                    .col(DeviceMetrics::DeviceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_device_metrics_timestamp")
                    .table((Alias::new("device"), DeviceMetrics::Table))
                    .col(DeviceMetrics::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table((Alias::new("device"), DeviceMetrics::Table)).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table((Alias::new("device"), Devices::Table)).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Devices {
    Table,
    Id,
    Name,
    DeviceType,
    Protocol,
    Status,
    Metadata,
    CreatedAt,
    UpdatedAt,
    LastSeenAt,
}

#[derive(DeriveIden)]
enum DeviceMetrics {
    Table,
    Id,
    DeviceId,
    MetricName,
    Value,
    Timestamp,
}
