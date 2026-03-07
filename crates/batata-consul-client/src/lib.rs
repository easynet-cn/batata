//! Consul-compatible HTTP client SDK
//!
//! This crate provides a Rust client for the Consul HTTP API v1,
//! supporting KV store, service registration, health checks,
//! catalog queries, sessions, events, and cluster status.
//!
//! Features:
//! - ACL token authentication via `X-Consul-Token` header
//! - Blocking queries with `WaitIndex` / `WaitTime`
//! - QueryOptions/QueryMeta for all read operations
//! - WriteOptions/WriteMeta for all write operations

pub mod acl;
pub mod agent;
pub mod catalog;
pub mod client;
pub mod config;
pub mod error;
pub mod event;
pub mod health;
pub mod kv;
pub mod model;
pub mod session;
pub mod status;

pub use client::ConsulClient;
pub use config::ConsulClientConfig;
pub use error::{ConsulError, Result};
pub use model::*;
