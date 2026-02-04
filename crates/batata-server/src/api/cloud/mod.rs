//! Cloud Native Integration API
//!
//! This module provides integration with cloud-native platforms:
//! - Kubernetes service sync (bidirectional)
//! - Prometheus service discovery

pub mod kubernetes;
pub mod model;
pub mod prometheus;

// Re-export key types
pub use kubernetes::{K8sServiceSync, K8sSyncConfig, configure as configure_kubernetes};
pub use prometheus::{PrometheusServiceDiscovery, configure as configure_prometheus};
