//! Config cache data for tracking per-config state and listeners

use std::sync::Arc;

use md5::{Digest, Md5};

use super::listener::{ConfigChangeEventListener, ConfigChangeListener};

/// Cache entry for a single config item, tracking content, MD5, and listeners.
pub struct CacheData {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    /// Config type (e.g., "properties", "yaml", "json") for change parsing
    pub config_type: String,
    pub listeners: Vec<Arc<dyn ConfigChangeListener>>,
    /// Listeners that receive detailed field-level change events
    pub change_event_listeners: Vec<Arc<dyn ConfigChangeEventListener>>,
    pub is_listening: bool,
}

impl CacheData {
    /// Create a new CacheData for the given config key.
    pub fn new(data_id: &str, group: &str, tenant: &str) -> Self {
        Self {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: String::new(),
            md5: String::new(),
            config_type: String::new(),
            listeners: Vec::new(),
            change_event_listeners: Vec::new(),
            is_listening: false,
        }
    }

    /// Update the content and recompute the MD5 hash.
    /// Returns `true` if the content actually changed.
    pub fn update_content(&mut self, content: &str) -> bool {
        let new_md5 = compute_md5(content);
        if new_md5 != self.md5 {
            self.content = content.to_string();
            self.md5 = new_md5;
            true
        } else {
            false
        }
    }

    /// Add a listener to this cache entry.
    pub fn add_listener(&mut self, listener: Arc<dyn ConfigChangeListener>) {
        self.listeners.push(listener);
    }

    /// Remove all listeners (returns the count of removed listeners).
    pub fn remove_all_listeners(&mut self) -> usize {
        let count = self.listeners.len();
        self.listeners.clear();
        count
    }

    /// Check if there are any registered listeners.
    pub fn has_listeners(&self) -> bool {
        !self.listeners.is_empty()
    }

    /// Build the cache key for this config.
    pub fn key(&self) -> String {
        build_cache_key(&self.data_id, &self.group, &self.tenant)
    }
}

/// Build a cache key from config identifiers.
pub fn build_cache_key(data_id: &str, group: &str, tenant: &str) -> String {
    if tenant.is_empty() {
        format!("{}+{}", data_id, group)
    } else {
        format!("{}+{}+{}", data_id, group, tenant)
    }
}

/// Compute MD5 hash of a string, returning the hex digest.
pub fn compute_md5(content: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_md5() {
        let md5 = compute_md5("hello world");
        assert_eq!(md5, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_compute_md5_empty() {
        let md5 = compute_md5("");
        assert_eq!(md5, "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn test_build_cache_key() {
        assert_eq!(build_cache_key("data-id", "group", ""), "data-id+group");
        assert_eq!(
            build_cache_key("data-id", "group", "tenant"),
            "data-id+group+tenant"
        );
    }

    #[test]
    fn test_cache_data_update_content() {
        let mut cache = CacheData::new("test", "DEFAULT_GROUP", "");
        assert!(cache.md5.is_empty());

        // First update: content changes
        assert!(cache.update_content("hello"));
        assert_eq!(cache.content, "hello");
        assert!(!cache.md5.is_empty());

        // Same content: no change
        assert!(!cache.update_content("hello"));

        // Different content: change
        assert!(cache.update_content("world"));
        assert_eq!(cache.content, "world");
    }

    #[test]
    fn test_cache_data_key() {
        let cache = CacheData::new("my-config", "DEFAULT_GROUP", "public");
        assert_eq!(cache.key(), "my-config+DEFAULT_GROUP+public");
    }

    #[test]
    fn test_cache_data_key_no_tenant() {
        let cache = CacheData::new("my-config", "DEFAULT_GROUP", "");
        assert_eq!(cache.key(), "my-config+DEFAULT_GROUP");
    }

    #[test]
    fn test_cache_data_new_defaults() {
        let cache = CacheData::new("id", "group", "tenant");
        assert_eq!(cache.data_id, "id");
        assert_eq!(cache.group, "group");
        assert_eq!(cache.tenant, "tenant");
        assert!(cache.content.is_empty());
        assert!(cache.md5.is_empty());
        assert!(cache.listeners.is_empty());
        assert!(!cache.is_listening);
    }

    #[test]
    fn test_cache_data_listeners() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let mut cache = CacheData::new("id", "group", "");
        assert!(!cache.has_listeners());

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let listener = Arc::new(super::super::listener::FnConfigChangeListener::new(
            move |_info: super::super::listener::ConfigResponse| {
                called_clone.store(true, Ordering::SeqCst);
            },
        ));

        cache.add_listener(listener.clone());
        assert!(cache.has_listeners());
        assert_eq!(cache.listeners.len(), 1);

        // Add another
        cache.add_listener(listener);
        assert_eq!(cache.listeners.len(), 2);

        // Remove all
        let removed = cache.remove_all_listeners();
        assert_eq!(removed, 2);
        assert!(!cache.has_listeners());
    }

    #[test]
    fn test_cache_data_md5_consistency() {
        let mut cache = CacheData::new("id", "group", "");
        cache.update_content("test content");

        let expected_md5 = compute_md5("test content");
        assert_eq!(cache.md5, expected_md5);
    }

    #[test]
    fn test_compute_md5_different_content() {
        let md5a = compute_md5("content-a");
        let md5b = compute_md5("content-b");
        assert_ne!(md5a, md5b);
    }

    #[test]
    fn test_build_cache_key_special_chars() {
        assert_eq!(
            build_cache_key("data.id", "my-group", "ns:public"),
            "data.id+my-group+ns:public"
        );
    }
}
