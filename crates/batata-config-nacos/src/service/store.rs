//! In-memory Nacos config store

use std::sync::Arc;

use dashmap::DashMap;

use crate::model::NacosConfig;

use super::cache::ConfigCacheService;
use super::notifier::ConfigChangeNotifier;

/// In-memory Nacos config store
///
/// Provides CRUD operations with integrated cache and notification.
/// For production use, this would be backed by a database via the
/// persistence trait. This implementation is for embedded/testing mode.
#[derive(Clone)]
pub struct NacosConfigServiceImpl {
    /// Key: "namespace+group+dataId", Value: config
    configs: Arc<DashMap<String, NacosConfig>>,
    /// MD5 cache for fast change detection
    cache: ConfigCacheService,
    /// Change notifier for long-polling
    notifier: ConfigChangeNotifier,
}

impl NacosConfigServiceImpl {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(DashMap::new()),
            cache: ConfigCacheService::new(),
            notifier: ConfigChangeNotifier::new(),
        }
    }

    /// Access the cache service
    pub fn cache(&self) -> &ConfigCacheService {
        &self.cache
    }

    /// Access the notifier
    pub fn notifier(&self) -> &ConfigChangeNotifier {
        &self.notifier
    }

    /// Get a config
    pub fn get_config(&self, namespace: &str, group: &str, data_id: &str) -> Option<NacosConfig> {
        let key = ConfigCacheService::build_key(namespace, group, data_id);
        self.configs.get(&key).map(|c| c.clone())
    }

    /// Publish (create or update) a config
    pub fn publish_config(&self, mut config: NacosConfig) -> bool {
        let key = ConfigCacheService::build_key(&config.namespace, &config.group, &config.data_id);

        // Calculate MD5
        config.refresh_md5();
        let now = chrono::Utc::now().timestamp_millis();
        config.modified_at = now;

        let is_create = !self.configs.contains_key(&key);
        if is_create {
            config.created_at = now;
        }

        // Update cache
        self.cache.dump(
            &config.namespace,
            &config.group,
            &config.data_id,
            &config.md5,
            now,
            &config.content_type,
        );

        // Store
        self.configs.insert(key, config.clone());

        // Notify listeners
        self.notifier
            .notify_change(&config.namespace, &config.group, &config.data_id);

        true
    }

    /// Delete a config
    pub fn delete_config(&self, namespace: &str, group: &str, data_id: &str) -> bool {
        let key = ConfigCacheService::build_key(namespace, group, data_id);

        if self.configs.remove(&key).is_some() {
            self.cache.remove(namespace, group, data_id);
            self.notifier.notify_change(namespace, group, data_id);
            true
        } else {
            false
        }
    }

    /// Search configs by namespace and optional group/data_id filter
    pub fn search_configs(
        &self,
        namespace: &str,
        group: Option<&str>,
        data_id_pattern: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> (Vec<NacosConfig>, u64) {
        let prefix = format!("{namespace}+");

        let mut results: Vec<NacosConfig> = self
            .configs
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| entry.value().clone())
            .filter(|config| {
                group.map(|g| config.group == g).unwrap_or(true)
                    && data_id_pattern
                        .map(|p| config.data_id.contains(p))
                        .unwrap_or(true)
            })
            .collect();

        results.sort_by(|a, b| a.data_id.cmp(&b.data_id));
        let total = results.len() as u64;
        let start = ((page.saturating_sub(1)) * page_size) as usize;
        let paged: Vec<NacosConfig> = results
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        (paged, total)
    }

    /// Check which listen items have changed (compare client MD5 with server MD5)
    pub fn check_listen_items(
        &self,
        items: &[crate::model::ListenItem],
    ) -> Vec<crate::model::ListenItem> {
        items
            .iter()
            .filter(|item| {
                let server_md5 = self
                    .cache
                    .get_md5(&item.namespace, &item.group, &item.data_id);
                match server_md5 {
                    Some(md5) => md5 != item.md5,
                    None => !item.md5.is_empty(), // Config deleted but client has MD5
                }
            })
            .cloned()
            .collect()
    }

    /// Get total config count
    pub fn config_count(&self) -> usize {
        self.configs.len()
    }
}

impl Default for NacosConfigServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ListenItem;

    fn test_config(ns: &str, group: &str, data_id: &str, content: &str) -> NacosConfig {
        NacosConfig {
            data_id: data_id.to_string(),
            group: group.to_string(),
            namespace: ns.to_string(),
            content: content.to_string(),
            content_type: "yaml".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_publish_and_get() {
        let store = NacosConfigServiceImpl::new();
        let config = test_config("public", "DEFAULT_GROUP", "app.yaml", "key: value");

        assert!(store.publish_config(config));

        let result = store.get_config("public", "DEFAULT_GROUP", "app.yaml");
        assert!(result.is_some());
        let cfg = result.unwrap();
        assert_eq!(cfg.content, "key: value");
        assert!(!cfg.md5.is_empty());
        assert!(cfg.created_at > 0);
        assert!(cfg.modified_at > 0);
    }

    #[test]
    fn test_delete() {
        let store = NacosConfigServiceImpl::new();
        store.publish_config(test_config(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "content",
        ));

        assert!(store.delete_config("public", "DEFAULT_GROUP", "app.yaml"));
        assert!(
            store
                .get_config("public", "DEFAULT_GROUP", "app.yaml")
                .is_none()
        );
        assert!(!store.delete_config("public", "DEFAULT_GROUP", "nonexistent"));
    }

    #[test]
    fn test_search() {
        let store = NacosConfigServiceImpl::new();
        store.publish_config(test_config("public", "DEFAULT_GROUP", "app.yaml", "a"));
        store.publish_config(test_config("public", "DEFAULT_GROUP", "db.yaml", "b"));
        store.publish_config(test_config("other-ns", "DEFAULT_GROUP", "app.yaml", "c"));

        let (results, total) = store.search_configs("public", None, None, 1, 10);
        assert_eq!(total, 2);
        assert_eq!(results.len(), 2);

        // Filter by data_id pattern
        let (results, total) = store.search_configs("public", None, Some("app"), 1, 10);
        assert_eq!(total, 1);
        assert_eq!(results[0].data_id, "app.yaml");
    }

    #[test]
    fn test_listen_detects_changes() {
        let store = NacosConfigServiceImpl::new();
        store.publish_config(test_config("public", "DEFAULT_GROUP", "app.yaml", "v1"));

        let cfg = store
            .get_config("public", "DEFAULT_GROUP", "app.yaml")
            .unwrap();
        let current_md5 = cfg.md5.clone();

        // Client has current MD5 → no change
        let items = vec![ListenItem {
            namespace: "public".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            data_id: "app.yaml".to_string(),
            md5: current_md5,
        }];
        let changed = store.check_listen_items(&items);
        assert!(changed.is_empty());

        // Client has outdated MD5 → change detected
        let items = vec![ListenItem {
            namespace: "public".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            data_id: "app.yaml".to_string(),
            md5: "old_md5".to_string(),
        }];
        let changed = store.check_listen_items(&items);
        assert_eq!(changed.len(), 1);
    }

    #[test]
    fn test_cache_synced_on_publish() {
        let store = NacosConfigServiceImpl::new();
        store.publish_config(test_config(
            "public",
            "DEFAULT_GROUP",
            "app.yaml",
            "content",
        ));

        let cached_md5 = store.cache().get_md5("public", "DEFAULT_GROUP", "app.yaml");
        let stored_md5 = store
            .get_config("public", "DEFAULT_GROUP", "app.yaml")
            .unwrap()
            .md5;

        assert_eq!(cached_md5, Some(stored_md5));
    }
}
