//! Cloud Native Integration Plugin for Batata
//!
//! This crate provides integration with cloud-native platforms:
//! - Kubernetes service sync (bidirectional)
//! - Prometheus service discovery

#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

pub mod kubernetes;
pub mod model;
pub mod prometheus;

// Re-export key types
pub use kubernetes::{K8sServiceSync, K8sSyncConfig, configure as configure_kubernetes};
pub use prometheus::{PrometheusServiceDiscovery, configure as configure_prometheus};
