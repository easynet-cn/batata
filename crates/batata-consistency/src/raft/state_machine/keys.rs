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

    /// Build a prefix for scanning config history keys by their leading
    /// segments (`tenant@@group@@data_id`). A history key can only be
    /// prefix-matched from its leading segments, so empty *leading* segments
    /// must fall back to an empty prefix (scan everything). `tenant` is the
    /// primary scope: if it is empty we cannot restrict by `group`/`data_id`
    /// either, so the prefix is empty.
    pub fn config_history_prefix(tenant: &str, group: &str, data_id: &str) -> String {
        if tenant.is_empty() {
            return String::new();
        }
        let mut prefix = format!("{}@@", tenant);
        if !group.is_empty() {
            prefix.push_str(&format!("{}@@", group));
            if !data_id.is_empty() {
                prefix.push_str(&format!("{}@@", data_id));
            }
        }
        prefix
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

#[cfg(test)]
mod tests {
    use super::RocksStateMachine;

    #[test]
    fn history_prefix_is_empty_when_all_filters_are_blank() {
        // The console audit handler searches with ("", "", ""). Building a
        // literal "@@@@" prefix would match no history key at all.
        assert_eq!(RocksStateMachine::config_history_prefix("", "", ""), "");
        let keys = [
            RocksStateMachine::config_history_key("app.yml", "DEFAULT_GROUP", "public", 1),
            RocksStateMachine::config_history_key("app.yml", "DEFAULT_GROUP", "", 2),
        ];
        assert!(
            keys.iter()
                .all(|k| k.starts_with(&RocksStateMachine::config_history_prefix("", "", "")))
        );
    }

    #[test]
    fn history_prefix_stops_at_the_first_empty_segment() {
        assert_eq!(
            RocksStateMachine::config_history_prefix("public", "", ""),
            "public@@"
        );
        // A dataId cannot be appended while group is blank: the key layout is
        // positional, so skipping group would match unrelated tenants' rows.
        assert_eq!(
            RocksStateMachine::config_history_prefix("public", "", "app.yml"),
            "public@@"
        );
        assert_eq!(
            RocksStateMachine::config_history_prefix("public", "DEFAULT_GROUP", ""),
            "public@@DEFAULT_GROUP@@"
        );
        // An empty tenant cannot be narrowed by later segments either.
        assert_eq!(
            RocksStateMachine::config_history_prefix("", "DEFAULT_GROUP", "app.yml"),
            ""
        );
    }

    #[test]
    fn full_history_prefix_is_a_real_key_prefix() {
        let prefix = RocksStateMachine::config_history_prefix("public", "DEFAULT_GROUP", "app.yml");
        assert_eq!(prefix, "public@@DEFAULT_GROUP@@app.yml@@");
        let key = RocksStateMachine::config_history_key("app.yml", "DEFAULT_GROUP", "public", 42);
        assert!(key.starts_with(&prefix));
        let other =
            RocksStateMachine::config_history_key("other.yml", "DEFAULT_GROUP", "public", 43);
        assert!(!other.starts_with(&prefix));
    }
}
