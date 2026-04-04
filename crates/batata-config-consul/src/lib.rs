//! Consul-compatible KV Store
//!
//! Implements configuration management with Consul-native semantics:
//! - **Flat key paths** (e.g., "config/app/database/url")
//! - **Blocking queries** (long-polling with index-based change detection)
//! - **CAS** (Compare-And-Swap) for optimistic concurrency
//! - **Sessions** for distributed locking and ephemeral keys
//! - **Transactions** for atomic multi-key operations
//! - **Base64-encoded** values
//!
//! This crate is completely independent from `batata-config-nacos`.
//! It depends only on `batata-foundation` for infrastructure traits.

pub mod error;
pub mod model;
pub mod service;
pub mod traits;
