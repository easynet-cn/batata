//! Registry module - centralized service and server registration
//!
//! This module provides registry types for managing services and servers
//! in a type-safe manner using TypeId-based lookups.

/// `server_registry` module.
#[cfg(test)]
pub mod server_registry;
/// `service_registry` module.
#[cfg(test)]
pub mod service_registry;

/// Re-exported item.
#[cfg(test)]
pub use server_registry::ServerRegistry;
/// Re-exported item.
#[cfg(test)]
pub use service_registry::ServiceRegistry;
