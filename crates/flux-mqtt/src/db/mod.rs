// 数据库实体模块
// 仅在启用 persistence 特性时编译

#[cfg(feature = "persistence")]
pub mod mqtt_session;

#[cfg(feature = "persistence")]
pub mod mqtt_offline_message;

#[cfg(feature = "persistence")]
pub mod mqtt_retained_message;

#[cfg(feature = "persistence")]
pub mod mqtt_acl_rule;
