//! Nacos-compatible Naming Service
//!
//! Implements service discovery with Nacos-native semantics:
//! - **Namespace → Group → Service** hierarchy
//! - **Ephemeral** instances (client heartbeat) vs **Persistent** instances (server-managed)
//! - **Cluster** configuration with per-cluster health check settings
//! - **Protection threshold** to prevent cascading failures
//! - **Selector** for label-based and expression-based instance filtering
//!
//! This crate is completely independent from `batata-naming-consul`.
//! It depends only on `batata-foundation` for infrastructure traits.

pub mod adapter;
pub mod error;
pub mod model;
pub mod service;
pub mod traits;
