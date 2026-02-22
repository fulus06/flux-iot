use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

/// 设备指令实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "device_commands")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub device_id: String,
    pub command_type: String,
    pub params: Option<Json>,
    pub timeout_seconds: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<Json>,
    pub error: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::command_response::Entity")]
    Responses,
}

impl Related<super::command_response::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Responses.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// 指令响应实体
pub mod command_response {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "command_responses")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub command_id: String,
        pub device_id: String,
        pub response_data: Json,
        pub received_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::Entity",
            from = "Column::CommandId",
            to = "super::Column::Id"
        )]
        Command,
    }

    impl Related<super::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Command.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// 场景实体
pub mod scene {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "scenes")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub name: String,
        pub description: Option<String>,
        pub triggers: Json,
        pub conditions: Option<Json>,
        pub actions: Json,
        pub enabled: bool,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::scene_execution::Entity")]
        Executions,
    }

    impl Related<super::scene_execution::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Executions.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// 场景执行历史实体
pub mod scene_execution {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "scene_executions")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub scene_id: String,
        pub trigger_type: String,
        pub executed_at: DateTime<Utc>,
        pub success: bool,
        pub error: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::scene::Entity",
            from = "Column::SceneId",
            to = "super::scene::Column::Id"
        )]
        Scene,
    }

    impl Related<super::scene::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Scene.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}
