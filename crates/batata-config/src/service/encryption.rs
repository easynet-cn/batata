//! Configuration encryption service
//!
//! Provides encryption/decryption for sensitive configuration values.
//! Uses AES-GCM encryption from batata-common.

use std::sync::Arc;

use tracing::{debug, warn};

use batata_common::crypto::{AesGcmEncryptionPlugin, CryptoResult, EncryptionPlugin};

/// Configuration encryption service
///
/// This service wraps the encryption plugin and provides a simple interface
/// for encrypting and decrypting configuration content.
pub struct ConfigEncryptionService {
    plugin: Arc<dyn EncryptionPlugin>,
}

impl ConfigEncryptionService {
    /// Create a new encryption service with the given encryption key.
    ///
    /// If the key is empty, encryption will be disabled.
    pub fn new(encryption_key: &str) -> CryptoResult<Self> {
        let plugin = AesGcmEncryptionPlugin::new(encryption_key)?;
        Ok(Self {
            plugin: Arc::new(plugin),
        })
    }

    /// Create a disabled encryption service (no encryption)
    pub fn disabled() -> Self {
        Self {
            plugin: Arc::new(AesGcmEncryptionPlugin::disabled()),
        }
    }

    /// Check if encryption is enabled
    pub fn is_enabled(&self) -> bool {
        self.plugin.is_enabled()
    }

    /// Encrypt content if the content should be encrypted (based on data_id pattern)
    ///
    /// Returns (possibly encrypted content, encrypted_data_key)
    /// If encryption is disabled or the data_id doesn't match encryption pattern,
    /// returns the original content with an empty data key.
    pub async fn encrypt_if_needed(
        &self,
        data_id: &str,
        content: &str,
    ) -> (String, String) {
        // Check if this config should be encrypted
        // Convention: configs with data_id containing "cipher-" prefix are encrypted
        if !self.is_enabled() || !should_encrypt(data_id) {
            return (content.to_string(), String::new());
        }

        match self.plugin.encrypt(content).await {
            Ok((encrypted, data_key)) => {
                debug!("Encrypted config content for data_id={}", data_id);
                (encrypted, data_key)
            }
            Err(e) => {
                warn!("Failed to encrypt config {}: {}", data_id, e);
                (content.to_string(), String::new())
            }
        }
    }

    /// Decrypt content if it has an encrypted data key
    ///
    /// If the encrypted_data_key is empty, returns the content as-is.
    pub async fn decrypt_if_needed(
        &self,
        data_id: &str,
        content: &str,
        encrypted_data_key: &str,
    ) -> String {
        if encrypted_data_key.is_empty() || !self.is_enabled() {
            return content.to_string();
        }

        match self.plugin.decrypt(content, encrypted_data_key).await {
            Ok(decrypted) => {
                debug!("Decrypted config content for data_id={}", data_id);
                decrypted
            }
            Err(e) => {
                warn!("Failed to decrypt config {}: {}", data_id, e);
                content.to_string()
            }
        }
    }

    /// Encrypt content directly (for explicit encryption requests)
    pub async fn encrypt(&self, content: &str) -> CryptoResult<(String, String)> {
        self.plugin.encrypt(content).await
    }

    /// Decrypt content directly (for explicit decryption requests)
    pub async fn decrypt(
        &self,
        content: &str,
        encrypted_data_key: &str,
    ) -> CryptoResult<String> {
        self.plugin.decrypt(content, encrypted_data_key).await
    }
}

/// Check if a config data_id should be encrypted
///
/// Convention: data_ids with "cipher-" prefix are encrypted
fn should_encrypt(data_id: &str) -> bool {
    data_id.starts_with("cipher-")
}

/// Builder for ConfigEncryptionService with custom rules
pub struct ConfigEncryptionServiceBuilder {
    encryption_key: String,
    encrypt_patterns: Vec<String>,
}

impl ConfigEncryptionServiceBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            encryption_key: String::new(),
            encrypt_patterns: vec!["cipher-".to_string()],
        }
    }

    /// Set the encryption key
    pub fn encryption_key(mut self, key: &str) -> Self {
        self.encryption_key = key.to_string();
        self
    }

    /// Add a pattern for data_ids that should be encrypted
    pub fn add_pattern(mut self, pattern: &str) -> Self {
        self.encrypt_patterns.push(pattern.to_string());
        self
    }

    /// Build the encryption service
    pub fn build(self) -> CryptoResult<ConfigEncryptionService> {
        ConfigEncryptionService::new(&self.encryption_key)
    }
}

impl Default for ConfigEncryptionServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_common::crypto::ConfigEncryptionService as CryptoService;

    #[tokio::test]
    async fn test_disabled_service() {
        let service = ConfigEncryptionService::disabled();
        assert!(!service.is_enabled());

        let (encrypted, key) = service
            .encrypt_if_needed("cipher-test", "secret")
            .await;
        assert_eq!(encrypted, "secret");
        assert!(key.is_empty());
    }

    #[tokio::test]
    async fn test_encryption_service() {
        let crypto_key = CryptoService::generate_base64_key();
        let service = ConfigEncryptionService::new(&crypto_key).unwrap();

        assert!(service.is_enabled());

        // Test non-cipher data_id (should not encrypt)
        let (content, key) = service
            .encrypt_if_needed("normal-config", "value")
            .await;
        assert_eq!(content, "value");
        assert!(key.is_empty());

        // Test cipher data_id (should encrypt)
        let (encrypted, key) = service
            .encrypt_if_needed("cipher-database-password", "secret123")
            .await;
        assert_ne!(encrypted, "secret123");
        assert!(!key.is_empty());

        // Test decryption
        let decrypted = service
            .decrypt_if_needed("cipher-database-password", &encrypted, &key)
            .await;
        assert_eq!(decrypted, "secret123");
    }

    #[test]
    fn test_should_encrypt() {
        assert!(should_encrypt("cipher-password"));
        assert!(should_encrypt("cipher-secret-key"));
        assert!(!should_encrypt("normal-config"));
        assert!(!should_encrypt("password")); // doesn't have prefix
    }
}
