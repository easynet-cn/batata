// Unified abstraction layer for service discovery and configuration management.
// Protocol-specific types (Consul, Eureka, etc.) live in their respective plugin crates.

/// Configuration management abstraction.
pub mod config;
/// Service discovery abstraction.
pub mod discovery;
/// Health check abstraction.
pub mod health;
/// Common types shared across registry implementations.
pub mod types;

// Re-export main traits and types
pub use config::{
    BatchConfigStore, ConfigChangeEvent, ConfigError, ConfigStore, ConfigWatch, ImportPolicy,
    ImportResult,
};
pub use discovery::{BatchServiceDiscovery, DiscoveryError, ServiceDiscovery, ServiceSubscription};
pub use health::{
    HealthCheckManager, HealthError, HeartbeatManager, HeartbeatResponse, HeartbeatStatus,
    ServiceHealthSummary,
};
pub use types::*;
