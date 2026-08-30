//! Column family name constants for Consul RocksDB storage.
//!
//! All Consul-related column families are defined here as the single source of truth.
//! All CFs are managed by the core Raft group via ConsulRaftPluginHandler.

// KV and Session
/// The `CF_CONSUL_KV` constant.
pub const CF_CONSUL_KV: &str = "consul_kv";
/// The `CF_CONSUL_SESSIONS` constant.
pub const CF_CONSUL_SESSIONS: &str = "consul_sessions";

// ACL
/// The `CF_CONSUL_ACL` constant.
pub const CF_CONSUL_ACL: &str = "consul_acl";

// Prepared Queries
/// The `CF_CONSUL_QUERIES` constant.
pub const CF_CONSUL_QUERIES: &str = "consul_queries";

// Config Entries
/// The `CF_CONSUL_CONFIG_ENTRIES` constant.
pub const CF_CONSUL_CONFIG_ENTRIES: &str = "consul_config_entries";

// Connect CA and Intentions
/// The `CF_CONSUL_CA_ROOTS` constant.
pub const CF_CONSUL_CA_ROOTS: &str = "consul_ca_roots";
/// The `CF_CONSUL_INTENTIONS` constant.
pub const CF_CONSUL_INTENTIONS: &str = "consul_intentions";

// Network Coordinates
/// The `CF_CONSUL_COORDINATES` constant.
pub const CF_CONSUL_COORDINATES: &str = "consul_coordinates";

// Cluster Peering
/// The `CF_CONSUL_PEERING` constant.
pub const CF_CONSUL_PEERING: &str = "consul_peering";

// Operator (autopilot, keyring, raft servers)
/// The `CF_CONSUL_OPERATOR` constant.
pub const CF_CONSUL_OPERATOR: &str = "consul_operator";

// User Events (gossip-based, kept for future use)
/// The `CF_CONSUL_EVENTS` constant.
pub const CF_CONSUL_EVENTS: &str = "consul_events";

// Namespaces
/// The `CF_CONSUL_NAMESPACES` constant.
pub const CF_CONSUL_NAMESPACES: &str = "consul_namespaces";

// Partitions (simplified Enterprise feature)
/// The `CF_CONSUL_PARTITIONS` constant.
pub const CF_CONSUL_PARTITIONS: &str = "consul_partitions";

// Catalog (service registrations)
/// The `CF_CONSUL_CATALOG` constant.
pub const CF_CONSUL_CATALOG: &str = "consul_catalog";

// Health check configurations (persisted for restart recovery)
/// The `CF_CONSUL_HEALTH_CHECKS` constant.
pub const CF_CONSUL_HEALTH_CHECKS: &str = "consul_health_checks";
