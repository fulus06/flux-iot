use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "mqtt_offline_messages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub client_id: String,
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: i16,
    pub retained: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
