//! Consul-compatible HTTP client SDK
//!
//! This crate provides a Rust client for the Consul HTTP API v1,
//! supporting KV store, service registration, health checks,
//! catalog queries, sessions, events, cluster status, snapshots,
//! transactions, coordinates, locks/semaphores, and prepared queries.
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
pub mod config_entry;
pub mod connect;
pub mod coordinate;
pub mod error;
pub mod event;
pub mod health;
pub mod kv;
pub mod lock;
pub mod metrics;
pub mod model;
pub mod operator;
pub mod peering;
pub mod query;
pub mod session;
pub mod snapshot;
pub mod status;
pub mod txn;
pub mod watch;

pub use client::ConsulClient;
pub use config::ConsulClientConfig;
pub use error::{ConsulError, Result};
pub use model::*;
