//! Config cache service for fast MD5-based change detection
//!
//! Provides an in-memory cache of config MD5 hashes and metadata,
//! following the Nacos CacheItem/ConfigCacheService pattern.
//! This enables:
//! - Fast config listener MD5 comparison without DB roundtrips
//! - Quick startup by loading cache from persistence
//! - Accurate change detection for long-polling notifications

use md5::Digest;
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use tracing::{debug, info, warn};

/// Cached metadata for a single configuration item.
/// Equivalent to Nacos CacheItem.
#[derive(Debug, Clone)]
pub struct CacheItem {
    /// MD5 hash of the config content
    pub md5: String,
    /// Last modification timestamp (millis since epoch)
    pub last_modified: i64,
    /// Encrypted data key (for encrypted configs)
    pub encrypted_data_key: String,
    /// Config type (e.g., "yaml", "json", "properties")
    pub config_type: String,
    /// Gray/beta config entries keyed by gray_name
    pub gray_items: DashMap<String, GrayCacheItem>,
}

/// Cached gray/beta config item
#[derive(Debug, Clone)]
pub struct GrayCacheItem {
    /// MD5 hash of the gray config content
    pub md5: String,
    /// Last modification timestamp (millis since epoch)
    pub last_modified: i64,
    /// Encrypted data key (for encrypted configs)
    pub encrypted_data_key: String,
    /// Gray rule expression
    pub gray_rule: String,
}

/// In-memory config cache for fast MD5 lookups.
///
/// Thread-safe via DashMap. Used by:
/// - Config listener (long-polling) to compare MD5 without DB query
/// - Config publish to update cache after successful write
/// - Startup to pre-populate from persistence layer
pub struct ConfigCacheService {
    /// Cache of group_key -> CacheItem
    /// Key format: "{tenant}+{group}+{dataId}" (same as Nacos)
    cache: DashMap<String, CacheItem>,
}

impl ConfigCacheService {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Build cache key from components (Nacos groupKey format, pre-allocated)
    pub fn build_key(tenant: &str, group: &str, data_id: &str) -> String {
        let mut key = String::with_capacity(tenant.len() + group.len() + data_id.len() + 2);
        key.push_str(tenant);
        key.push('+');
        key.push_str(group);
        key.push('+');
        key.push_str(data_id);
        key
    }

    /// Dump (update) a config entry in the cache.
    ///
    /// Follows Nacos ConfigCacheService.dump() logic:
    /// 1. If MD5 changed -> update cache
    /// 2. If only timestamp is newer -> update timestamp
    /// 3. If outdated -> ignore
    #[allow(clippy::too_many_arguments)]
    pub fn dump(
        &self,
        tenant: &str,
        group: &str,
        data_id: &str,
        content: &str,
        last_modified: i64,
        config_type: &str,
        encrypted_data_key: &str,
    ) -> bool {
        let key = Self::build_key(tenant, group, data_id);
        let new_md5 = const_hex::encode(md5::Md5::digest(content.as_bytes()));

        if let Some(mut existing) = self.cache.get_mut(&key) {
            // Check if this update is outdated
            if existing.last_modified > last_modified {
                return false;
            }

            if existing.md5 != new_md5 {
                // Content changed
                existing.md5 = new_md5;
                existing.last_modified = last_modified;
                existing.config_type = config_type.to_string();
                existing.encrypted_data_key = encrypted_data_key.to_string();
                debug!(
                    "Config cache updated: {}/{}/{} (MD5 changed)",
                    tenant, group, data_id
                );
                return true;
            } else if existing.last_modified < last_modified {
                // Same content but newer timestamp
                existing.last_modified = last_modified;
                return false;
            }
            // No change at all
            return false;
        }

        // New entry
        self.cache.insert(
            key,
            CacheItem {
                md5: new_md5,
                last_modified,
                encrypted_data_key: encrypted_data_key.to_string(),
                config_type: config_type.to_string(),
                gray_items: DashMap::new(),
            },
        );
        true
    }

    /// Remove a config entry from the cache
    pub fn remove(&self, tenant: &str, group: &str, data_id: &str) {
        let key = Self::build_key(tenant, group, data_id);
        self.cache.remove(&key);
    }

    /// Update cache for a gray/beta config.
    ///
    /// If the parent config entry does not exist, a placeholder is created.
    #[allow(clippy::too_many_arguments)]
    pub fn dump_gray(
        &self,
        tenant: &str,
        group: &str,
        data_id: &str,
        gray_name: &str,
        content: &str,
        last_modified: i64,
        gray_rule: &str,
        encrypted_data_key: &str,
    ) {
        let key = Self::build_key(tenant, group, data_id);
        let new_md5 = const_hex::encode(md5::Md5::digest(content.as_bytes()));

        let entry = self.cache.entry(key).or_insert_with(|| CacheItem {
            md5: String::new(),
            last_modified: 0,
            encrypted_data_key: String::new(),
            config_type: String::new(),
            gray_items: DashMap::new(),
        });

        entry.gray_items.insert(
            gray_name.to_string(),
            GrayCacheItem {
                md5: new_md5,
                last_modified,
                encrypted_data_key: encrypted_data_key.to_string(),
                gray_rule: gray_rule.to_string(),
            },
        );

        debug!(
            "Gray config cache updated: {}/{}/{} gray={}",
            tenant, group, data_id, gray_name
        );
    }

    /// Remove a gray/beta config from cache.
    pub fn remove_gray(&self, tenant: &str, group: &str, data_id: &str, gray_name: &str) {
        let key = Self::build_key(tenant, group, data_id);
        if let Some(item) = self.cache.get(&key) {
            item.gray_items.remove(gray_name);
        }
    }

    /// Get the MD5 for a gray/beta config.
    pub fn get_gray_md5(
        &self,
        tenant: &str,
        group: &str,
        data_id: &str,
        gray_name: &str,
    ) -> Option<String> {
        let key = Self::build_key(tenant, group, data_id);
        self.cache
            .get(&key)
            .and_then(|item| item.gray_items.get(gray_name).map(|g| g.md5.clone()))
    }

    /// Get the MD5 for a config entry
    pub fn get_md5(&self, tenant: &str, group: &str, data_id: &str) -> Option<String> {
        let key = Self::build_key(tenant, group, data_id);
        self.cache.get(&key).map(|item| item.md5.clone())
    }

    /// Get cache item
    pub fn get(&self, tenant: &str, group: &str, data_id: &str) -> Option<CacheItem> {
        let key = Self::build_key(tenant, group, data_id);
        self.cache.get(&key).map(|item| item.clone())
    }

    /// Check if a config has changed by comparing MD5 (no allocation)
    pub fn has_changed(&self, tenant: &str, group: &str, data_id: &str, client_md5: &str) -> bool {
        let key = Self::build_key(tenant, group, data_id);
        match self.cache.get(&key) {
            Some(item) => item.md5 != client_md5,
            None => true, // Not in cache means it might be new or deleted
        }
    }

    /// Batch check MD5 changes for multiple configs.
    /// Returns list of (tenant, group, data_id, server_md5) for changed configs.
    pub fn batch_check_md5(
        &self,
        configs: &[(String, String, String, String)], // (tenant, group, dataId, clientMd5)
    ) -> Vec<(String, String, String, String)> {
        configs
            .iter()
            .filter_map(|(tenant, group, data_id, client_md5)| {
                let key = Self::build_key(tenant, group, data_id);
                if let Some(item) = self.cache.get(&key) {
                    if item.md5 != *client_md5 {
                        Some((
                            tenant.clone(),
                            group.clone(),
                            data_id.clone(),
                            item.md5.clone(),
                        ))
                    } else {
                        None
                    }
                } else {
                    // Config not in cache -- treat as changed
                    Some((
                        tenant.clone(),
                        group.clone(),
                        data_id.clone(),
                        String::new(),
                    ))
                }
            })
            .collect()
    }

    /// Load all configs from persistence into cache (startup initialization).
    /// This is the equivalent of Nacos DumpService.dumpAll().
    pub async fn load_from_persistence(
        &self,
        persistence: &dyn batata_persistence::traits::PersistenceService,
    ) {
        info!("Loading config cache from persistence...");

        // Get all namespaces
        let namespaces = match persistence.namespace_find_all().await {
            Ok(ns) => ns,
            Err(e) => {
                warn!("Failed to load namespaces for cache: {}", e);
                return;
            }
        };

        let mut count = 0u64;

        // For each namespace, load all configs
        let mut namespace_ids: Vec<String> =
            namespaces.iter().map(|n| n.namespace_id.clone()).collect();
        // Always include default namespace
        if !namespace_ids.contains(&String::new()) && !namespace_ids.contains(&"public".to_string())
        {
            namespace_ids.push(String::new());
        }

        for ns_id in &namespace_ids {
            let tenant = if ns_id == "public" {
                ""
            } else {
                ns_id.as_str()
            };
            let configs = match persistence.config_find_by_namespace(tenant).await {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to load configs for namespace '{}': {}", ns_id, e);
                    continue;
                }
            };

            for config in &configs {
                let md5 = if config.md5.is_empty() {
                    const_hex::encode(md5::Md5::digest(config.content.as_bytes()))
                } else {
                    config.md5.clone()
                };

                let key = Self::build_key(&config.tenant, &config.group, &config.data_id);

                self.cache.insert(
                    key,
                    CacheItem {
                        md5,
                        last_modified: config.modified_time,
                        encrypted_data_key: config.encrypted_data_key.clone(),
                        config_type: config.config_type.clone(),
                        gray_items: DashMap::new(),
                    },
                );
                count += 1;
            }
        }

        info!(
            "Config cache loaded: {} entries from {} namespaces",
            count,
            namespace_ids.len()
        );
    }

    /// Export all cache entries as (key, md5, last_modified) tuples for reconciliation.
    /// Key format is "{tenant}+{group}+{dataId}".
    pub fn export_md5_digest(&self) -> Vec<(String, String, i64)> {
        self.cache
            .iter()
            .map(|entry| {
                (
                    entry.key().clone(),
                    entry.value().md5.clone(),
                    entry.value().last_modified,
                )
            })
            .collect()
    }

    /// Get the total number of cached entries
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Get current timestamp in millis
    pub fn now_millis() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }
}

impl Default for ConfigCacheService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_key() {
        assert_eq!(
            ConfigCacheService::build_key("public", "DEFAULT_GROUP", "app.yaml"),
            "public+DEFAULT_GROUP+app.yaml"
        );
        assert_eq!(ConfigCacheService::build_key("", "G1", "d1"), "+G1+d1");
    }

    #[test]
    fn test_dump_new_entry() {
        let cache = ConfigCacheService::new();
        let changed = cache.dump(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "key=value",
            1000,
            "properties",
            "",
        );
        assert!(changed);
        assert_eq!(cache.len(), 1);

        let md5 = cache.get_md5("public", "DEFAULT_GROUP", "app.yaml");
        assert!(md5.is_some());
        assert_eq!(
            md5.unwrap(),
            const_hex::encode(md5::Md5::digest("key=value"))
        );
    }

    #[test]
    fn test_dump_same_content_no_change() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d", "content", 1000, "text", "");
        let changed = cache.dump("t", "g", "d", "content", 1001, "text", "");
        assert!(!changed); // Same MD5, just newer timestamp
    }

    #[test]
    fn test_dump_different_content_is_change() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d", "old", 1000, "text", "");
        let changed = cache.dump("t", "g", "d", "new", 1001, "text", "");
        assert!(changed);
    }

    #[test]
    fn test_dump_outdated_ignored() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d", "content", 2000, "text", "");
        let changed = cache.dump("t", "g", "d", "newer", 1000, "text", ""); // older timestamp
        assert!(!changed);
        // Should still have the original MD5
        let md5 = cache.get_md5("t", "g", "d").unwrap();
        assert_eq!(md5, const_hex::encode(md5::Md5::digest("content")));
    }

    #[test]
    fn test_remove() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d", "content", 1000, "text", "");
        assert_eq!(cache.len(), 1);
        cache.remove("t", "g", "d");
        assert_eq!(cache.len(), 0);
        assert!(cache.get_md5("t", "g", "d").is_none());
    }

    #[test]
    fn test_has_changed() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d", "content", 1000, "text", "");
        let correct_md5 = const_hex::encode(md5::Md5::digest("content"));

        assert!(!cache.has_changed("t", "g", "d", &correct_md5));
        assert!(cache.has_changed("t", "g", "d", "wrong_md5"));
        assert!(cache.has_changed("t", "g", "nonexistent", &correct_md5));
    }

    #[test]
    fn test_batch_check_md5() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d1", "content1", 1000, "text", "");
        cache.dump("t", "g", "d2", "content2", 1000, "text", "");

        let md5_1 = const_hex::encode(md5::Md5::digest("content1"));

        let configs = vec![
            ("t".into(), "g".into(), "d1".into(), md5_1.clone()), // unchanged
            ("t".into(), "g".into(), "d2".into(), "wrong".into()), // changed
            ("t".into(), "g".into(), "d3".into(), "any".into()),  // not in cache
        ];

        let changed = cache.batch_check_md5(&configs);
        assert_eq!(changed.len(), 2); // d2 and d3
        assert!(changed.iter().any(|(_, _, d, _)| d == "d2"));
        assert!(changed.iter().any(|(_, _, d, _)| d == "d3"));
    }

    #[test]
    fn test_get_cache_item() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d", "content", 1000, "yaml", "enc_key");

        let item = cache.get("t", "g", "d").unwrap();
        assert_eq!(item.config_type, "yaml");
        assert_eq!(item.encrypted_data_key, "enc_key");
        assert_eq!(item.last_modified, 1000);
    }

    #[test]
    fn test_clear() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d1", "c1", 1000, "text", "");
        cache.dump("t", "g", "d2", "c2", 1000, "text", "");
        assert_eq!(cache.len(), 2);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_dump_gray() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d1", "base_content", 1000, "yaml", "");
        cache.dump_gray(
            "t",
            "g",
            "d1",
            "beta",
            "gray_content",
            1000,
            "beta_rule",
            "",
        );

        let item = cache.get("t", "g", "d1").unwrap();
        assert_eq!(item.gray_items.len(), 1);

        let gray = item.gray_items.get("beta").unwrap();
        assert_eq!(
            gray.md5,
            const_hex::encode(md5::Md5::digest("gray_content"))
        );
        assert_eq!(gray.gray_rule, "beta_rule");
    }

    #[test]
    fn test_dump_gray_creates_parent_if_missing() {
        let cache = ConfigCacheService::new();
        // Dump gray without a parent entry
        cache.dump_gray("t", "g", "d1", "beta", "gray_content", 1000, "rule", "");

        assert_eq!(cache.len(), 1);
        let item = cache.get("t", "g", "d1").unwrap();
        assert!(item.md5.is_empty()); // Placeholder parent
        assert_eq!(item.gray_items.len(), 1);
    }

    #[test]
    fn test_remove_gray() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d1", "content", 1000, "yaml", "");
        cache.dump_gray("t", "g", "d1", "beta", "gray", 1000, "rule", "");
        cache.remove_gray("t", "g", "d1", "beta");

        let item = cache.get("t", "g", "d1").unwrap();
        assert_eq!(item.gray_items.len(), 0);
    }

    #[test]
    fn test_get_gray_md5() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d1", "content", 1000, "yaml", "");
        cache.dump_gray("t", "g", "d1", "beta", "gray_content", 1000, "rule", "");

        let md5 = cache.get_gray_md5("t", "g", "d1", "beta");
        assert_eq!(
            md5,
            Some(const_hex::encode(md5::Md5::digest("gray_content")))
        );

        assert!(cache.get_gray_md5("t", "g", "d1", "nonexistent").is_none());
        assert!(cache.get_gray_md5("t", "g", "missing", "beta").is_none());
    }

    #[test]
    fn test_multiple_gray_configs() {
        let cache = ConfigCacheService::new();
        cache.dump("t", "g", "d1", "content", 1000, "yaml", "");
        cache.dump_gray(
            "t",
            "g",
            "d1",
            "beta",
            "beta_content",
            1000,
            "beta_rule",
            "",
        );
        cache.dump_gray(
            "t",
            "g",
            "d1",
            "canary",
            "canary_content",
            1000,
            "canary_rule",
            "",
        );

        let item = cache.get("t", "g", "d1").unwrap();
        assert_eq!(item.gray_items.len(), 2);

        let beta = item.gray_items.get("beta").unwrap();
        assert_eq!(beta.gray_rule, "beta_rule");

        let canary = item.gray_items.get("canary").unwrap();
        assert_eq!(canary.gray_rule, "canary_rule");
    }
}
