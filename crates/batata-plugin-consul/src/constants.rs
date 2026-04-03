//! Column family name constants for Consul RocksDB storage.
//!
//! All Consul-related column families are defined here as the single source of truth.
//! Both the standalone RocksDB mode and the Consul Raft state machine reference these constants.

// Core column families (used by Consul Raft state machine)
pub const CF_CONSUL_KV: &str = "consul_kv";
pub const CF_CONSUL_ACL: &str = "consul_acl";
pub const CF_CONSUL_SESSIONS: &str = "consul_sessions";
pub const CF_CONSUL_QUERIES: &str = "consul_queries";

// Extended column families (used by standalone RocksDB services)
pub const CF_CONSUL_CONFIG_ENTRIES: &str = "consul_config_entries";
pub const CF_CONSUL_CA_ROOTS: &str = "consul_ca_roots";
pub const CF_CONSUL_INTENTIONS: &str = "consul_intentions";
pub const CF_CONSUL_COORDINATES: &str = "consul_coordinates";
pub const CF_CONSUL_PEERING: &str = "consul_peering";
pub const CF_CONSUL_OPERATOR: &str = "consul_operator";
pub const CF_CONSUL_EVENTS: &str = "consul_events";
