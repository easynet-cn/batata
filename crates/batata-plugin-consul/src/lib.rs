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

pub mod api;
pub mod constants;
pub mod consul_meta;
pub mod plugin;
pub mod raft;

pub mod acl;
pub mod agent;
pub mod catalog;
pub mod check_index;
pub mod config_entry;
pub mod connect;
pub mod connect_ca;
pub mod coordinate;
pub mod event;
pub mod health;
pub mod index_provider;
pub mod internal;
pub mod kv;
pub mod lock;
pub mod model;
pub mod namespace;
pub mod naming_store;
pub mod operator;
pub mod peering;
pub mod query;
pub mod result_handler;
pub mod route;
pub mod session;
pub mod snapshot;
pub mod status;

// Re-export route functions for easy integration
pub use route::routes;

// Re-export the plugin
pub use plugin::ConsulPlugin;

// Re-export key services
pub use acl::AclService;
pub use agent::ConsulAgentService;
pub use catalog::ConsulCatalogService;
pub use config_entry::ConsulConfigEntryService;
pub use event::{ConsulEventService, EventBroadcaster};
pub use health::ConsulHealthService;
pub use kv::ConsulKVService;
pub use lock::{ConsulLockService, ConsulSemaphoreService};
pub use operator::{ConsulOperatorService, ConsulOperatorServiceReal};
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
pub use peering::ConsulPeeringService;
pub use raft::plugin_handler::{CONSUL_PLUGIN_ID, ConsulRaftPluginHandler, ConsulRaftWriter};
pub use result_handler::ConsulResultHandler;
