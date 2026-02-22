use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "mqtt_acl_rules")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub client_id: Option<String>,
    pub username: Option<String>,
    pub topic_pattern: String,
    pub action: String,
    pub permission: String,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
