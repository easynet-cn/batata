use async_trait::async_trait;

use batata_common::crypto::{CryptoResult, EncryptionPlugin};

/// Pass-through encryption plugin: returns plaintext unchanged and reports
/// `is_enabled() == false`. Useful as an explicit "no encryption" backend
/// that can be named in configuration (`algorithm: none`) instead of
/// omitting the key entirely, and as an alternative impl proving the
/// registry supports multiple concrete backends.
pub struct NoopEncryptionPlugin;

impl NoopEncryptionPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopEncryptionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EncryptionPlugin for NoopEncryptionPlugin {
    fn name(&self) -> &str {
        "none"
    }

    async fn encrypt(&self, plaintext: &str) -> CryptoResult<(String, String)> {
        Ok((plaintext.to_string(), String::new()))
    }

    async fn decrypt(&self, ciphertext: &str, _encrypted_data_key: &str) -> CryptoResult<String> {
        Ok(ciphertext.to_string())
    }

    fn is_enabled(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_passes_through() {
        let p = NoopEncryptionPlugin::new();
        assert_eq!(p.name(), "none");
        assert!(!p.is_enabled());
        let (out, key) = p.encrypt("secret").await.unwrap();
        assert_eq!(out, "secret");
        assert!(key.is_empty());
        let back = p.decrypt("secret", "").await.unwrap();
        assert_eq!(back, "secret");
    }
}
