use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // MQTT 客户端表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("mqtt"), MqttClients::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MqttClients::ClientId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MqttClients::Username).string())
                    .col(ColumnDef::new(MqttClients::PasswordHash).string())
                    .col(
                        ColumnDef::new(MqttClients::Connected)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(MqttClients::ConnectedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(MqttClients::DisconnectedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(MqttClients::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // MQTT 订阅表
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("mqtt"), MqttSubscriptions::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MqttSubscriptions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MqttSubscriptions::ClientId).string().not_null())
                    .col(ColumnDef::new(MqttSubscriptions::Topic).string().not_null())
                    .col(
                        ColumnDef::new(MqttSubscriptions::Qos)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MqttSubscriptions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mqtt_subscriptions_client_id")
                            .from((Alias::new("mqtt"), MqttSubscriptions::Table), MqttSubscriptions::ClientId)
                            .to((Alias::new("mqtt"), MqttClients::Table), MqttClients::ClientId)
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
                    .name("idx_mqtt_clients_connected")
                    .table((Alias::new("mqtt"), MqttClients::Table))
                    .col(MqttClients::Connected)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mqtt_subscriptions_client_id")
                    .table((Alias::new("mqtt"), MqttSubscriptions::Table))
                    .col(MqttSubscriptions::ClientId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mqtt_subscriptions_topic")
                    .table((Alias::new("mqtt"), MqttSubscriptions::Table))
                    .col(MqttSubscriptions::Topic)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table((Alias::new("mqtt"), MqttSubscriptions::Table)).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table((Alias::new("mqtt"), MqttClients::Table)).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MqttClients {
    Table,
    ClientId,
    Username,
    PasswordHash,
    Connected,
    ConnectedAt,
    DisconnectedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum MqttSubscriptions {
    Table,
    Id,
    ClientId,
    Topic,
    Qos,
    CreatedAt,
}
