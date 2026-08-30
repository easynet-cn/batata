//! Builder module - trait-driven application building
//!
//! This module provides a builder pattern for constructing the Batata server
//! application with clear separation of concerns across different phases.

/// `config_builder` module.
pub mod config_builder;
/// `persistence_builder` module.
pub mod persistence_builder;
/// `service_builder` module.
pub mod service_builder;
/// `server_builder` module.
pub mod server_builder;
/// `app_builder` module.
pub mod app_builder;

/// Re-exported item.
pub use config_builder::ConfigBuilder;
/// Re-exported item.
pub use persistence_builder::PersistenceBuilder;
/// Re-exported item.
pub use service_builder::ServiceBuilder;
/// Re-exported item.
pub use server_builder::ServerBuilder;
/// Re-exported item.
pub use app_builder::AppBuilder;
