//! Batata Foundation — Shared infrastructure traits and types
//!
//! This crate defines the Layer 1 abstractions that are shared across all
//! registry implementations (Nacos, Consul, etc.). It contains NO domain-specific
//! concepts like namespace, group, datacenter, service, or instance.
//!
//! # Design Principle
//!
//! **Share infrastructure, isolate domain.**
//!
//! - Storage engines, consistency protocols, cluster membership, and event buses
//!   are infrastructure concerns that every registry implementation needs.
//! - Service models, config models, health check semantics, and query filters
//!   are domain concerns that differ between Nacos, Consul, etcd, etc.
//!
//! This crate provides the former. Domain crates build on top of these traits
//! with their own models and logic.

pub mod cluster;
pub mod connection;
pub mod consistency;
pub mod error;
pub mod event;
pub mod lifecycle;
pub mod storage;
pub mod types;

pub use error::FoundationError;
