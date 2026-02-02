//! Configuration encryption service
//!
//! Provides encryption/decryption for sensitive configuration values.
//! Uses AES-GCM encryption from batata-common.

use std::sync::Arc;

use regex::Regex;
use tracing::{debug, warn};

use batata_common::crypto::{AesGcmEncryptionPlugin, CryptoResult, EncryptionPlugin};

/// Pattern for determining which configs should be encrypted
#[derive(Clone, Debug)]
pub enum EncryptionPattern {
    /// Match by prefix (e.g., "cipher-")
    Prefix(String),
    /// Match by suffix (e.g., "-secret")
    Suffix(String),
    /// Match by substring (e.g., "password")
    Contains(String),
    /// Match by exact name
    Exact(String),
    /// Match by regex pattern
    Regex(String),
}

impl EncryptionPattern {
    /// Check if a data_id matches this pattern
    pub fn matches(&self, data_id: &str) -> bool {
        match self {
            EncryptionPattern::Prefix(prefix) => data_id.starts_with(prefix),
            EncryptionPattern::Suffix(suffix) => data_id.ends_with(suffix),
            EncryptionPattern::Contains(substring) => data_id.contains(substring),
            EncryptionPattern::Exact(exact) => data_id == exact,
            EncryptionPattern::Regex(pattern) => {
                if let Ok(re) = Regex::new(pattern) {
                    re.is_match(data_id)
                } else {
                    false
                }
            }
        }
    }
}

/// Configuration encryption service
///
/// This service wraps the encryption plugin and provides a simple interface
/// for encrypting and decrypting configuration content.
pub struct ConfigEncryptionService {
    plugin: Arc<dyn EncryptionPlugin>,
    /// Patterns that determine which configs should be encrypted
    patterns: Vec<EncryptionPattern>,
}

impl ConfigEncryptionService {
    /// Create a new encryption service with the given encryption key.
    ///
    /// If the key is empty, encryption will be disabled.
    /// Uses default pattern: prefix "cipher-"
    pub fn new(encryption_key: &str) -> CryptoResult<Self> {
        let plugin = AesGcmEncryptionPlugin::new(encryption_key)?;
        Ok(Self {
            plugin: Arc::new(plugin),
            patterns: vec![EncryptionPattern::Prefix("cipher-".to_string())],
        })
    }

    /// Create a new encryption service with custom patterns
    pub fn with_patterns(
        encryption_key: &str,
        patterns: Vec<EncryptionPattern>,
    ) -> CryptoResult<Self> {
        let plugin = AesGcmEncryptionPlugin::new(encryption_key)?;
        Ok(Self {
            plugin: Arc::new(plugin),
            patterns,
        })
    }

    /// Create a disabled encryption service (no encryption)
    pub fn disabled() -> Self {
        Self {
            plugin: Arc::new(AesGcmEncryptionPlugin::disabled()),
            patterns: vec![],
        }
    }

    /// Check if encryption is enabled
    pub fn is_enabled(&self) -> bool {
        self.plugin.is_enabled()
    }

    /// Get the current encryption patterns
    pub fn patterns(&self) -> &[EncryptionPattern] {
        &self.patterns
    }

    /// Check if a data_id should be encrypted based on configured patterns
    pub fn should_encrypt(&self, data_id: &str) -> bool {
        self.patterns.iter().any(|p| p.matches(data_id))
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
        if !self.is_enabled() || !self.should_encrypt(data_id) {
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

/// Builder for ConfigEncryptionService with custom rules
pub struct ConfigEncryptionServiceBuilder {
    encryption_key: String,
    patterns: Vec<EncryptionPattern>,
}

impl ConfigEncryptionServiceBuilder {
    /// Create a new builder with default cipher- prefix pattern
    pub fn new() -> Self {
        Self {
            encryption_key: String::new(),
            patterns: vec![EncryptionPattern::Prefix("cipher-".to_string())],
        }
    }

    /// Create a new builder with no default patterns
    pub fn empty() -> Self {
        Self {
            encryption_key: String::new(),
            patterns: vec![],
        }
    }

    /// Set the encryption key
    pub fn encryption_key(mut self, key: &str) -> Self {
        self.encryption_key = key.to_string();
        self
    }

    /// Add a prefix pattern (e.g., "cipher-" matches "cipher-password")
    pub fn prefix_pattern(mut self, prefix: &str) -> Self {
        self.patterns.push(EncryptionPattern::Prefix(prefix.to_string()));
        self
    }

    /// Add a suffix pattern (e.g., "-secret" matches "db-secret")
    pub fn suffix_pattern(mut self, suffix: &str) -> Self {
        self.patterns.push(EncryptionPattern::Suffix(suffix.to_string()));
        self
    }

    /// Add a contains pattern (e.g., "password" matches "db-password-config")
    pub fn contains_pattern(mut self, substring: &str) -> Self {
        self.patterns.push(EncryptionPattern::Contains(substring.to_string()));
        self
    }

    /// Add an exact match pattern
    pub fn exact_pattern(mut self, exact: &str) -> Self {
        self.patterns.push(EncryptionPattern::Exact(exact.to_string()));
        self
    }

    /// Add a regex pattern (e.g., "^secret-.*-key$")
    pub fn regex_pattern(mut self, regex: &str) -> Self {
        self.patterns.push(EncryptionPattern::Regex(regex.to_string()));
        self
    }

    /// Add a custom encryption pattern
    pub fn pattern(mut self, pattern: EncryptionPattern) -> Self {
        self.patterns.push(pattern);
        self
    }

    /// Clear all patterns
    pub fn clear_patterns(mut self) -> Self {
        self.patterns.clear();
        self
    }

    /// Build the encryption service
    pub fn build(self) -> CryptoResult<ConfigEncryptionService> {
        ConfigEncryptionService::with_patterns(&self.encryption_key, self.patterns)
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
    fn test_encryption_pattern_prefix() {
        let pattern = EncryptionPattern::Prefix("cipher-".to_string());
        assert!(pattern.matches("cipher-password"));
        assert!(pattern.matches("cipher-secret-key"));
        assert!(!pattern.matches("password"));
        assert!(!pattern.matches("my-cipher-config"));
    }

    #[test]
    fn test_encryption_pattern_suffix() {
        let pattern = EncryptionPattern::Suffix("-secret".to_string());
        assert!(pattern.matches("db-secret"));
        assert!(pattern.matches("api-key-secret"));
        assert!(!pattern.matches("secret-key"));
        assert!(!pattern.matches("secretfile"));
    }

    #[test]
    fn test_encryption_pattern_contains() {
        let pattern = EncryptionPattern::Contains("password".to_string());
        assert!(pattern.matches("db-password"));
        assert!(pattern.matches("password-config"));
        assert!(pattern.matches("user-password-hash"));
        assert!(!pattern.matches("db-secret"));
    }

    #[test]
    fn test_encryption_pattern_exact() {
        let pattern = EncryptionPattern::Exact("secret-config".to_string());
        assert!(pattern.matches("secret-config"));
        assert!(!pattern.matches("secret-config-v2"));
        assert!(!pattern.matches("my-secret-config"));
    }

    #[test]
    fn test_encryption_pattern_regex() {
        let pattern = EncryptionPattern::Regex(r"^secret-.*-key$".to_string());
        assert!(pattern.matches("secret-api-key"));
        assert!(pattern.matches("secret-db-key"));
        assert!(!pattern.matches("secret-key"));
        assert!(!pattern.matches("my-secret-api-key"));
    }

    #[test]
    fn test_service_should_encrypt() {
        let crypto_key = CryptoService::generate_base64_key();
        let service = ConfigEncryptionService::new(&crypto_key).unwrap();

        assert!(service.should_encrypt("cipher-password"));
        assert!(service.should_encrypt("cipher-secret-key"));
        assert!(!service.should_encrypt("normal-config"));
        assert!(!service.should_encrypt("password")); // doesn't have prefix
    }

    #[test]
    fn test_builder_with_custom_patterns() {
        let crypto_key = CryptoService::generate_base64_key();
        let service = ConfigEncryptionServiceBuilder::empty()
            .encryption_key(&crypto_key)
            .suffix_pattern("-secret")
            .contains_pattern("password")
            .build()
            .unwrap();

        // Should match suffix pattern
        assert!(service.should_encrypt("db-secret"));
        // Should match contains pattern
        assert!(service.should_encrypt("my-password-config"));
        // Should not match default cipher- prefix (we used empty builder)
        assert!(!service.should_encrypt("cipher-key"));
    }

    #[test]
    fn test_builder_with_regex_pattern() {
        let crypto_key = CryptoService::generate_base64_key();
        let service = ConfigEncryptionServiceBuilder::empty()
            .encryption_key(&crypto_key)
            .regex_pattern(r"^(prod|staging)-secrets?-.*$")
            .build()
            .unwrap();

        assert!(service.should_encrypt("prod-secret-api-key"));
        assert!(service.should_encrypt("prod-secrets-config"));
        assert!(service.should_encrypt("staging-secret-db"));
        assert!(!service.should_encrypt("dev-secret-key"));
        assert!(!service.should_encrypt("production-secrets"));
    }

    #[tokio::test]
    async fn test_custom_patterns_encryption() {
        let crypto_key = CryptoService::generate_base64_key();
        let service = ConfigEncryptionServiceBuilder::empty()
            .encryption_key(&crypto_key)
            .suffix_pattern("-secret")
            .build()
            .unwrap();

        // Should encrypt with suffix pattern
        let (encrypted, key) = service
            .encrypt_if_needed("db-secret", "password123")
            .await;
        assert_ne!(encrypted, "password123");
        assert!(!key.is_empty());

        // Should not encrypt without matching pattern
        let (content, key2) = service
            .encrypt_if_needed("normal-config", "value")
            .await;
        assert_eq!(content, "value");
        assert!(key2.is_empty());
    }
}
