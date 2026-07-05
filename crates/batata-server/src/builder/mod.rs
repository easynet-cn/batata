//! Builder module - trait-driven application building
//!
//! This module provides a builder pattern for constructing the Batata server
//! application with clear separation of concerns across different phases.

pub mod config_builder;
pub mod persistence_builder;
pub mod service_builder;
pub mod server_builder;
pub mod app_builder;

pub use config_builder::ConfigBuilder;
pub use persistence_builder::PersistenceBuilder;
pub use service_builder::ServiceBuilder;
pub use server_builder::ServerBuilder;
pub use app_builder::AppBuilder;
