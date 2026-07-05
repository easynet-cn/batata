//! Registry module - centralized service and server registration
//!
//! This module provides registry types for managing services and servers
//! in a type-safe manner using TypeId-based lookups.

pub mod service_registry;
pub mod server_registry;

pub use service_registry::ServiceRegistry;
pub use server_registry::ServerRegistry;
