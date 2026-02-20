pub mod connection;
pub mod coordinator;
pub mod resource;
pub mod signal;
pub mod state;

pub use connection::{ConnectionGuard, ConnectionTracker};
pub use coordinator::{ShutdownCoordinator, ShutdownCoordinatorBuilder, ShutdownPhase};
pub use resource::{DatabaseResource, FileResource, Resource, ResourceError, ResourceManager};
pub use signal::{ShutdownSignal, SignalHandler};
pub use state::{StateError, StateManager};
