//! RocksDB key builders for the Raft state machine.
//!
//! These are associated functions on `RocksStateMachine` (kept that way
//! so external call sites like `RocksStateMachine::config_key(...)` do
//! not need to change). Split out of `mod.rs` into their own `impl` block
//! for file-size / review hygiene — no behavioral change.

use super::RocksStateMachine;

impl RocksStateMachine {
    /// Generate config key
    pub fn config_key(data_id: &str, group: &str, tenant: &str) -> String {
        format!("{}@@{}@@{}", tenant, group, data_id)
    }

    /// Generate config history key
    pub fn config_history_key(data_id: &str, group: &str, tenant: &str, id: u64) -> String {
        format!("{}@@{}@@{}@@{}", tenant, group, data_id, id)
    }

    /// Generate config gray key
    pub fn config_gray_key(data_id: &str, group: &str, tenant: &str, gray_name: &str) -> String {
        format!("{}@@{}@@{}@@{}", tenant, group, data_id, gray_name)
    }

    /// Generate namespace key
    pub fn namespace_key(namespace_id: &str) -> String {
        namespace_id.to_string()
    }

    /// Generate user key
    pub fn user_key(username: &str) -> String {
        username.to_string()
    }

    /// Generate role key
    pub fn role_key(role: &str, username: &str) -> String {
        format!("{}@@{}", role, username)
    }

    /// Generate permission key
    pub fn permission_key(role: &str, resource: &str, action: &str) -> String {
        format!("{}@@{}@@{}", role, resource, action)
    }

    /// Generate instance key
    pub fn instance_key(
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    ) -> String {
        format!(
            "{}@@{}@@{}@@{}",
            namespace_id, group_name, service_name, instance_id
        )
    }

    /// Generate lock key
    pub fn lock_key(namespace: &str, name: &str) -> String {
        format!("{}::{}", namespace, name)
    }
}
