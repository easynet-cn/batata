//! Registry module - centralized service and server registration
//!
//! This module provides registry types for managing services and servers
//! in a type-safe manner using TypeId-based lookups.

/// `server_registry` module.
pub mod server_registry;
/// `service_registry` module.
pub mod service_registry;

/// Re-exported item.
pub use server_registry::ServerRegistry;
/// Re-exported item.
pub use service_registry::ServiceRegistry;
