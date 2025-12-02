pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;

// Re-export for tests
pub use processor::Processor;
pub use state::GsnInfo;
pub use error::GsnError;

#[cfg(not(feature = "exclude_entrypoint_deprecated"))]
pub mod entrypoint;
