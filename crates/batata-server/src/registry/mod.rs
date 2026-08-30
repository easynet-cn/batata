//! Registry module - centralized service and server registration
//!
//! This module provides registry types for managing services and servers
//! in a type-safe manner using TypeId-based lookups.

/// `service_registry` module.
pub mod service_registry;
/// `server_registry` module.
pub mod server_registry;

/// Re-exported item.
pub use service_registry::ServiceRegistry;
/// Re-exported item.
pub use server_registry::ServerRegistry;
