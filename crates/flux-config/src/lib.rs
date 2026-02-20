pub mod global;
pub mod loader;
pub mod protocol;
pub mod recording;
pub mod timeshift;

pub use global::{GlobalConfig, StorageGlobalConfig, StoragePoolConfig, SystemConfig};
pub use loader::ConfigLoader;
pub use protocol::{ProtocolConfig, ProtocolStorageConfig};
pub use recording::{RecordingConfig, RecordingSegmentConfig, RecordingCompressionConfig, RecordingQualityConfig, RecordingConversionConfig};
pub use timeshift::{TimeShiftGlobalConfig, TimeShiftMergedConfig, TimeShiftProtocolConfig};
