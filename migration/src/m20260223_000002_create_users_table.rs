use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table((Alias::new("public"), Users::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Users::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Users::Username)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Users::PasswordHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Users::Roles)
                            .json_binary()
                            .not_null()
                            .default("'[]'::jsonb"),
                    )
                    .col(
                        ColumnDef::new(Users::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Users::UpdatedAt)
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_users_username")
                    .table((Alias::new("public"), Users::Table))
                    .col(Users::Username)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_users_enabled")
                    .table((Alias::new("public"), Users::Table))
                    .col(Users::Enabled)
                    .to_owned(),
            )
            .await?;

        // 插入默认用户
        let insert = Query::insert()
            .into_table((Alias::new("public"), Users::Table))
            .columns([
                Users::Id,
                Users::Username,
                Users::PasswordHash,
                Users::Roles,
                Users::Enabled,
            ])
            .values_panic([
                "admin-default".into(),
                "admin".into(),
                "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyYqNqJqGqKm".into(),
                "'[\"admin\"]'::jsonb".into(),
                true.into(),
            ])
            .on_conflict(
                OnConflict::column(Users::Username)
                    .do_nothing()
                    .to_owned(),
            )
            .to_owned();

        manager.exec_stmt(insert).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table((Alias::new("public"), Users::Table)).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    Username,
    PasswordHash,
    Roles,
    Enabled,
    CreatedAt,
    UpdatedAt,
}
