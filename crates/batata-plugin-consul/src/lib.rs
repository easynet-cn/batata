//! Consul-compatible API plugin for Batata
//!
//! This crate provides Consul-compatible HTTP API endpoints that can be integrated
//! with the Batata service discovery and configuration platform.
//!
//! ## Modules
//! - `acl`: Access Control List management
//! - `agent`: Service agent operations (register, deregister)
//! - `catalog`: Service catalog queries
//! - `config_entry`: Config entries management (service-defaults, proxy-defaults, etc.)
//! - `event`: User event operations
//! - `health`: Health check operations
//! - `kv`: Key-Value store operations
//! - `lock`: Distributed lock and semaphore operations
//! - `model`: Data models for Consul API
//! - `operator`: Cluster operator endpoints (Raft, Autopilot, Keyring)
//! - `query`: Prepared query operations
//! - `route`: Route configuration for actix-web
//! - `session`: Distributed session/lock management
//! - `snapshot`: Snapshot save/restore operations
//! - `status`: Cluster status information
//! - `connect`: Service mesh (discovery chain, exported/imported services)
//! - `coordinate`: Network coordinate/RTT endpoints
//! - `peering`: Cluster peering for cross-datacenter service discovery

#![warn(missing_docs)]

/// The `api` module.
pub mod api;
/// The `constants` module.
pub mod constants;
/// The `consul_meta` module.
pub mod consul_meta;
/// The `plugin` module.
pub mod plugin;
/// The `raft` module.
pub mod raft;

/// The `acl` module.
pub mod acl;
/// The `acl_store` module.
pub mod acl_store;
/// The `agent` module.
pub mod agent;
/// The `api_metrics` module.
pub mod api_metrics;
/// The `catalog` module.
pub mod catalog;
/// The `check_index` module.
pub mod check_index;
/// The `config_entry` module.
pub mod config_entry;
/// The `connect` module.
pub mod connect;
/// The `connect_ca` module.
pub mod connect_ca;
/// The `coordinate` module.
pub mod coordinate;
/// The `event` module.
pub mod event;
/// The `filter` module.
pub mod filter;
/// The `health` module.
pub mod health;
/// The `index_provider` module.
pub mod index_provider;
/// The `internal` module.
pub mod internal;
/// The `kv` module.
pub mod kv;
/// The `log_broadcast` module.
pub mod log_broadcast;
/// The `model` module.
pub mod model;
/// The `namespace` module.
pub mod namespace;
/// The `naming_store` module.
pub mod naming_store;
/// The `oidc` module.
pub mod oidc;
/// The `operator` module.
pub mod operator;
/// The `partition` module.
pub mod partition;
/// The `peering` module.
pub mod peering;
/// The `query` module.
pub mod query;
/// The `result_handler` module.
pub mod result_handler;
/// The `route` module.
pub mod route;
/// The `session` module.
pub mod session;
/// The `snapshot` module.
pub mod snapshot;
/// The `snapshot_archive` module.
pub mod snapshot_archive;
/// The `status` module.
pub mod status;
/// The `vivaldi` module.
pub mod vivaldi;

// Re-export route functions for easy integration
pub use route::routes;

// Re-export the plugin and its config
pub use model::ConsulPluginConfig;
pub use plugin::ConsulPlugin;

// Re-export key services
pub use acl::AclService;
pub use agent::ConsulAgentService;
pub use catalog::ConsulCatalogService;
pub use config_entry::ConsulConfigEntryService;
pub use event::{ConsulEventService, EventBroadcaster};
pub use health::ConsulHealthService;
pub use kv::ConsulKVService;
pub use operator::ConsulOperatorService;
pub use query::ConsulQueryService;
pub use session::ConsulSessionService;
pub use snapshot::{ConsulSnapshotService, ConsulSnapshotServicePersistent};

// Tier 2 services
pub use connect::ConsulConnectService;
pub use connect_ca::ConsulConnectCAService;
pub use consul_meta::{
    ConsulQueryOptions, ConsulResponseMeta, consul_not_found, consul_ok, parse_go_duration,
    parse_go_duration_secs,
};
pub use coordinate::{ConsulCoordinateService, ConsulCoordinateServicePersistent};
pub use index_provider::ConsulIndexProvider;
pub use namespace::ConsulNamespaceService;
pub use naming_store::ConsulNamingStore;
pub use partition::ConsulPartitionService;
pub use peering::ConsulPeeringService;
pub use raft::plugin_handler::{CONSUL_PLUGIN_ID, ConsulRaftPluginHandler, ConsulRaftWriter};
pub use result_handler::ConsulResultHandler;
