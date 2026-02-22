use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime as ChronoDateTime, Utc};

/// 设备实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "devices")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub protocol: String,
    pub status: String,
    pub product_id: Option<String>,
    pub secret: Option<String>,
    pub metadata: Option<Json>,
    pub tags: Option<Json>,
    pub group_id: Option<String>,
    pub location: Option<Json>,
    pub created_at: ChronoDateTime<Utc>,
    pub updated_at: ChronoDateTime<Utc>,
    pub last_seen: Option<ChronoDateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::device_group::Entity",
        from = "Column::GroupId",
        to = "super::device_group::Column::Id"
    )]
    DeviceGroup,
}

impl Related<super::device_group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DeviceGroup.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub mod device {
    pub use super::*;
}

/// 设备分组实体
pub mod device_group {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "device_groups")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub name: String,
        pub description: Option<String>,
        pub parent_id: Option<String>,
        pub path: String,
        pub created_at: ChronoDateTime<Utc>,
        pub updated_at: ChronoDateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::device::Entity")]
        Device,
    }

    impl Related<super::device::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Device.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// 设备状态历史实体
pub mod device_status_history {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "device_status_history")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub device_id: String,
        pub status: String,
        pub timestamp: ChronoDateTime<Utc>,
        pub metadata: Option<Json>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::device::Entity",
            from = "Column::DeviceId",
            to = "super::device::Column::Id"
        )]
        Device,
    }

    impl Related<super::device::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Device.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// 设备指标实体
pub mod device_metrics {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "device_metrics")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub device_id: String,
        pub metric_name: String,
        pub metric_value: f64,
        pub unit: Option<String>,
        pub timestamp: ChronoDateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::device::Entity",
            from = "Column::DeviceId",
            to = "super::device::Column::Id"
        )]
        Device,
    }

    impl Related<super::device::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Device.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}
