use std::sync::{Arc, OnceLock};

use batata_common::crypto::EncryptionPlugin;
use dashmap::DashMap;

/// Holds all registered [`EncryptionPlugin`]s keyed by `name()`.
///
/// Mirrors Nacos' `EncryptionPluginManager`: lookup by algorithm name,
/// last-write-wins on name collision.
pub struct EncryptionPluginRegistry {
    plugins: DashMap<String, Arc<dyn EncryptionPlugin>>,
}

impl EncryptionPluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: DashMap::new(),
        }
    }

    pub fn register(&self, plugin: Arc<dyn EncryptionPlugin>) {
        let name = plugin.name().to_string();
        self.plugins.insert(name, plugin);
    }

    pub fn unregister(&self, name: &str) -> bool {
        self.plugins.remove(name).is_some()
    }

    pub fn find(&self, name: &str) -> Option<Arc<dyn EncryptionPlugin>> {
        self.plugins.get(name).map(|e| e.value().clone())
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.plugins.len());
        for entry in self.plugins.iter() {
            out.push(entry.key().clone());
        }
        out
    }
}

impl Default for EncryptionPluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL: OnceLock<Arc<EncryptionPluginRegistry>> = OnceLock::new();

/// Process-wide singleton registry. Startup code registers built-in plugins
/// here; config publish/read paths resolve them by algorithm name.
pub fn global_encryption_registry() -> Arc<EncryptionPluginRegistry> {
    GLOBAL
        .get_or_init(|| Arc::new(EncryptionPluginRegistry::new()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use batata_common::crypto::CryptoResult;

    struct StubPlugin {
        name: &'static str,
    }

    #[async_trait]
    impl EncryptionPlugin for StubPlugin {
        fn name(&self) -> &str {
            self.name
        }
        async fn encrypt(&self, plaintext: &str) -> CryptoResult<(String, String)> {
            Ok((plaintext.to_string(), String::new()))
        }
        async fn decrypt(&self, ciphertext: &str, _key: &str) -> CryptoResult<String> {
            Ok(ciphertext.to_string())
        }
        fn is_enabled(&self) -> bool {
            true
        }
    }

    #[test]
    fn register_find_unregister() {
        let reg = EncryptionPluginRegistry::new();
        reg.register(Arc::new(StubPlugin { name: "alpha" }));
        reg.register(Arc::new(StubPlugin { name: "beta" }));

        assert_eq!(reg.len(), 2);
        assert!(reg.find("alpha").is_some());
        assert_eq!(reg.find("alpha").unwrap().name(), "alpha");
        assert!(reg.find("missing").is_none());

        assert!(reg.unregister("alpha"));
        assert!(!reg.unregister("alpha"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn register_same_name_replaces() {
        let reg = EncryptionPluginRegistry::new();
        reg.register(Arc::new(StubPlugin { name: "dup" }));
        reg.register(Arc::new(StubPlugin { name: "dup" }));
        assert_eq!(reg.len(), 1);
    }
}
