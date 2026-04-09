//! Column family name constants for Consul RocksDB storage.
//!
//! All Consul-related column families are defined here as the single source of truth.
//! All CFs are managed by the core Raft group via ConsulRaftPluginHandler.

// KV and Session
pub const CF_CONSUL_KV: &str = "consul_kv";
pub const CF_CONSUL_SESSIONS: &str = "consul_sessions";

// ACL
pub const CF_CONSUL_ACL: &str = "consul_acl";

// Prepared Queries
pub const CF_CONSUL_QUERIES: &str = "consul_queries";

// Config Entries
pub const CF_CONSUL_CONFIG_ENTRIES: &str = "consul_config_entries";

// Connect CA and Intentions
pub const CF_CONSUL_CA_ROOTS: &str = "consul_ca_roots";
pub const CF_CONSUL_INTENTIONS: &str = "consul_intentions";

// Network Coordinates
pub const CF_CONSUL_COORDINATES: &str = "consul_coordinates";

// Cluster Peering
pub const CF_CONSUL_PEERING: &str = "consul_peering";

// Operator (autopilot, keyring, raft servers)
pub const CF_CONSUL_OPERATOR: &str = "consul_operator";

// User Events (gossip-based, kept for future use)
pub const CF_CONSUL_EVENTS: &str = "consul_events";

// Namespaces
pub const CF_CONSUL_NAMESPACES: &str = "consul_namespaces";

// Catalog (service registrations)
pub const CF_CONSUL_CATALOG: &str = "consul_catalog";

// Health check configurations (persisted for restart recovery)
pub const CF_CONSUL_HEALTH_CHECKS: &str = "consul_health_checks";
