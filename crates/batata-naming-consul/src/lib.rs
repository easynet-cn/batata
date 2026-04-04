//! Consul-compatible Naming Service
//!
//! Implements service discovery with Consul-native semantics:
//! - **Datacenter** as the top-level grouping (no namespace/group)
//! - **Agent** model for local service registration
//! - **Catalog** model for cluster-wide service discovery
//! - **Health checks** with tri-state: passing/warning/critical
//! - **Tags** for service classification (not metadata-based)
//! - **Connect** for service mesh integration
//!
//! This crate is completely independent from `batata-naming-nacos`.
//! It depends only on `batata-foundation` for infrastructure traits.

pub mod error;
pub mod model;
pub mod service;
pub mod traits;
