//! Integration test: prove `global_encryption_registry()` is a shared
//! singleton, and that plugins registered there can be resolved by name
//! from unrelated modules — which is exactly what the config startup path
//! relies on.

use std::sync::Arc;

use async_trait::async_trait;
use batata_common::crypto::{CryptoResult, EncryptionPlugin};
use batata_plugin::{NoopEncryptionPlugin, global_encryption_registry};

struct MarkerPlugin;

#[async_trait]
impl EncryptionPlugin for MarkerPlugin {
    fn name(&self) -> &str {
        "integration_marker"
    }
    async fn encrypt(&self, plaintext: &str) -> CryptoResult<(String, String)> {
        Ok((format!("<enc>{}</enc>", plaintext), "k".to_string()))
    }
    async fn decrypt(&self, ciphertext: &str, _key: &str) -> CryptoResult<String> {
        Ok(ciphertext
            .trim_start_matches("<enc>")
            .trim_end_matches("</enc>")
            .to_string())
    }
    fn is_enabled(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn registry_dispatches_by_name_across_modules() {
    let reg = global_encryption_registry();
    reg.register(Arc::new(NoopEncryptionPlugin::new()));
    reg.register(Arc::new(MarkerPlugin));

    // Fetching the singleton again must return the same registry state.
    let other_handle = global_encryption_registry();
    let resolved = other_handle
        .find("integration_marker")
        .expect("plugin must be visible through singleton");
    assert_eq!(resolved.name(), "integration_marker");

    let (ct, key) = resolved.encrypt("hello").await.unwrap();
    assert_eq!(ct, "<enc>hello</enc>");
    assert_eq!(key, "k");
    let pt = resolved.decrypt(&ct, &key).await.unwrap();
    assert_eq!(pt, "hello");

    // "none" must resolve to the noop plugin we registered.
    let noop = other_handle.find("none").expect("noop should be present");
    assert!(!noop.is_enabled());

    // Clean up.
    global_encryption_registry().unregister("integration_marker");
}
