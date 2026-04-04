//! MD5-based config cache for fast change detection

use dashmap::DashMap;

/// Cache item storing MD5 and metadata for a config
#[derive(Debug, Clone)]
pub struct CacheItem {
    /// MD5 hash of content
    pub md5: String,
    /// Last modification timestamp
    pub last_modified: i64,
    /// Config type (yaml, json, properties, text, etc.)
    pub config_type: String,
    /// Encrypted data key (for encrypted configs)
    pub encrypted_data_key: String,
}

/// In-memory config cache keyed by "namespace+group+dataId"
///
/// Used for fast MD5 comparison during long-polling listener requests,
/// avoiding database roundtrips for unchanged configs.
#[derive(Clone)]
pub struct ConfigCacheService {
    cache: DashMap<String, CacheItem>,
}

impl ConfigCacheService {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Build cache key: "namespace+group+dataId"
    pub fn build_key(namespace: &str, group: &str, data_id: &str) -> String {
        let mut key = String::with_capacity(namespace.len() + group.len() + data_id.len() + 2);
        key.push_str(namespace);
        key.push('+');
        key.push_str(group);
        key.push('+');
        key.push_str(data_id);
        key
    }

    /// Update cache with new config data.
    /// Returns true if MD5 changed (content update), false otherwise.
    pub fn dump(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
        md5: &str,
        last_modified: i64,
        config_type: &str,
    ) -> bool {
        let key = Self::build_key(namespace, group, data_id);

        let md5_changed = self
            .cache
            .get(&key)
            .map(|existing| {
                // Ignore outdated entries
                if existing.last_modified > last_modified {
                    return false;
                }
                existing.md5 != md5
            })
            .unwrap_or(true);

        self.cache.insert(
            key,
            CacheItem {
                md5: md5.to_string(),
                last_modified,
                config_type: config_type.to_string(),
                encrypted_data_key: String::new(),
            },
        );

        md5_changed
    }

    /// Get cached MD5 for a config
    pub fn get_md5(&self, namespace: &str, group: &str, data_id: &str) -> Option<String> {
        let key = Self::build_key(namespace, group, data_id);
        self.cache.get(&key).map(|item| item.md5.clone())
    }

    /// Get full cache item
    pub fn get(&self, namespace: &str, group: &str, data_id: &str) -> Option<CacheItem> {
        let key = Self::build_key(namespace, group, data_id);
        self.cache.get(&key).map(|item| item.clone())
    }

    /// Remove from cache
    pub fn remove(&self, namespace: &str, group: &str, data_id: &str) {
        let key = Self::build_key(namespace, group, data_id);
        self.cache.remove(&key);
    }

    /// Get all cached keys as (namespace, group, data_id)
    pub fn all_keys(&self) -> Vec<(String, String, String)> {
        self.cache
            .iter()
            .filter_map(|entry| {
                let key = entry.key();
                let mut parts = key.splitn(3, '+');
                let ns = parts.next()?.to_string();
                let group = parts.next()?.to_string();
                let data_id = parts.next()?.to_string();
                Some((ns, group, data_id))
            })
            .collect()
    }

    /// Number of cached configs
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
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
    fn test_cache_dump_and_get() {
        let cache = ConfigCacheService::new();

        let changed = cache.dump(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "abc123",
            1000,
            "yaml",
        );
        assert!(changed);

        let md5 = cache.get_md5("public", "DEFAULT_GROUP", "app.yaml");
        assert_eq!(md5, Some("abc123".to_string()));
    }

    #[test]
    fn test_cache_detects_change() {
        let cache = ConfigCacheService::new();

        cache.dump(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "md5_v1",
            1000,
            "yaml",
        );

        // Same MD5 → no change
        let changed = cache.dump(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "md5_v1",
            1001,
            "yaml",
        );
        assert!(!changed);

        // Different MD5 → change detected
        let changed = cache.dump(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "md5_v2",
            1002,
            "yaml",
        );
        assert!(changed);
    }

    #[test]
    fn test_cache_ignores_outdated() {
        let cache = ConfigCacheService::new();

        cache.dump(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "md5_v2",
            2000,
            "yaml",
        );

        // Older timestamp, different MD5 → ignored (no change reported)
        let changed = cache.dump(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "md5_v1",
            1000,
            "yaml",
        );
        assert!(!changed);
    }

    #[test]
    fn test_cache_all_keys() {
        let cache = ConfigCacheService::new();
        cache.dump("public", "group-a", "cfg1", "md5", 1000, "yaml");
        cache.dump("public", "group-b", "cfg2", "md5", 1000, "json");

        let keys = cache.all_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_cache_remove() {
        let cache = ConfigCacheService::new();
        cache.dump("public", "DEFAULT_GROUP", "app.yaml", "md5", 1000, "yaml");
        assert_eq!(cache.len(), 1);

        cache.remove("public", "DEFAULT_GROUP", "app.yaml");
        assert_eq!(cache.len(), 0);
        assert!(
            cache
                .get_md5("public", "DEFAULT_GROUP", "app.yaml")
                .is_none()
        );
    }
}
