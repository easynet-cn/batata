//! Builder module - trait-driven application building
//!
//! This module provides a builder pattern for constructing the Batata server
//! application with clear separation of concerns across different phases.

/// `app_builder` module.
pub mod app_builder;
/// `config_builder` module.
#[cfg(test)]
pub mod config_builder;
/// `persistence_builder` module.
#[cfg(test)]
pub mod persistence_builder;
/// `server_builder` module.
#[cfg(test)]
pub mod server_builder;
/// `service_builder` module.
#[cfg(test)]
pub mod service_builder;

/// Re-exported item.
pub use app_builder::AppBuilder;
/// Re-exported item.
#[cfg(test)]
pub use config_builder::ConfigBuilder;
/// Re-exported item.
#[cfg(test)]
pub use persistence_builder::PersistenceBuilder;
/// Re-exported item.
#[cfg(test)]
pub use server_builder::ServerBuilder;
/// Re-exported item.
#[cfg(test)]
pub use service_builder::ServiceBuilder;
