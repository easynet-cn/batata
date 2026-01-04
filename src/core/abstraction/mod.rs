// Unified abstraction layer for multi-registry support (Nacos, Consul, etc.)
// This module provides common traits and types for service discovery and configuration management

pub mod config;
pub mod discovery;
pub mod health;
pub mod types;

// Re-export main traits and types (excluding consul submodules to avoid conflicts)
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

// Consul-specific types under namespaced modules
pub mod consul_types {
    pub use super::config::consul::*;
    pub use super::discovery::consul::*;
    pub use super::health::consul::*;
}
