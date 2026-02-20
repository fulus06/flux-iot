// Export modules
pub mod file_source;
pub mod manager;
pub mod source;
pub mod validator;
pub mod version;

#[cfg(feature = "sqlite")]
pub mod sqlite_source;

#[cfg(feature = "postgres")]
pub mod postgres_source;

// Re-exports
pub use file_source::FileSource;
pub use manager::{ConfigChange, ConfigManager};
pub use source::{ConfigSource, ConfigWatcher};
pub use validator::{ConfigValidator, CustomRule, RangeRule, ValidationError, ValidationRule};
pub use version::{ConfigVersion, VersionManager};

#[cfg(feature = "sqlite")]
pub use sqlite_source::SqliteSource;

#[cfg(feature = "postgres")]
pub use postgres_source::PostgresSource;
