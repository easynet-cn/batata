//! Consul-compatible HTTP client SDK for Rust.
//!
//! Provides 1:1 method-level parity with HashiCorp Consul's Go SDK
//! (`github.com/hashicorp/consul/api`). Every module corresponds to a
//! Go SDK file under `api/*.go`:
//!
//! | Rust module               | Consul Go source                          |
//! |---------------------------|-------------------------------------------|
//! | [`acl`]                   | `api/acl.go` (+ legacy v1 in same file)   |
//! | [`agent`]                 | `api/agent.go`                            |
//! | [`catalog`]               | `api/catalog.go`                          |
//! | [`config_entry`]          | `api/config_entry.go` (typed access)      |
//! | [`config_entry_types`]    | `api/config_entry_*.go` (all variants)    |
//! | [`connect`]               | `api/connect*.go` + `connect_intention.go`|
//! | [`coordinate`]            | `api/coordinate.go`                       |
//! | [`debug`]                 | `api/debug.go` — pprof endpoints          |
//! | [`discovery_chain`]       | `api/discovery_chain.go`                  |
//! | [`event`]                 | `api/event.go`                            |
//! | [`exported_services`]     | `api/exported_services.go`                |
//! | [`health`]                | `api/health.go`                           |
//! | [`imported_services`]     | `api/imported_services.go`                |
//! | [`internal`]              | `api/internal.go`                         |
//! | [`kv`]                    | `api/kv.go`                               |
//! | [`lock`]                  | `api/lock.go` + `api/semaphore.go`        |
//! | [`namespace`]             | `api/namespace.go` (Enterprise)           |
//! | [`operator`]              | `api/operator*.go` (area/audit/segment/…) |
//! | [`partition`]             | `api/partition.go` (Enterprise)           |
//! | [`peering`]               | `api/peering.go`                          |
//! | [`query`]                 | `api/prepared_query.go`                   |
//! | [`raw`]                   | `api/raw.go` — escape hatch               |
//! | [`session`]               | `api/session.go`                          |
//! | [`snapshot`]              | `api/snapshot.go`                         |
//! | [`status`]                | `api/status.go`                           |
//! | [`txn`]                   | `api/txn.go` (KV + Node + Service + Check)|
//! | [`watch`]                 | `api/watch/` blocking-query helper        |
//!
//! Rust-specific additions (not in Go SDK):
//! - [`client`] — HTTP connection + TLS + retries
//! - [`config`] — ConsulClientConfig
//! - [`error`]  — ConsulError / Result
//! - [`metrics`] — client-side instrumentation

pub mod acl;
pub mod agent;
pub mod catalog;
pub mod client;
pub mod config;
pub mod config_entry;
pub mod config_entry_types;
pub mod connect;
pub mod coordinate;
pub mod debug;
pub mod discovery_chain;
pub mod error;
pub mod event;
pub mod exported_services;
pub mod health;
pub mod imported_services;
pub mod internal;
pub mod kv;
pub mod lock;
pub mod metrics;
pub mod model;
pub mod namespace;
pub mod operator;
pub mod partition;
pub mod peering;
pub mod query;
pub mod raw;
pub mod session;
pub mod snapshot;
pub mod status;
pub mod txn;
pub mod watch;

pub use client::ConsulClient;
pub use config::ConsulClientConfig;
pub use error::{ConsulError, Result};
pub use model::*;
