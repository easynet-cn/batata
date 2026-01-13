//! Configuration encryption module
//!
//! Provides AES-GCM encryption/decryption for configuration data.
//! Compatible with Nacos encryption plugin interface.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::{rngs::OsRng, Rng, RngCore};
use thiserror::Error;

/// Error types for encryption operations
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Base64 decode error: {0}")]
    Base64Error(String),
}

/// Result type for crypto operations
pub type CryptoResult<T> = Result<T, CryptoError>;

/// Configuration encryption service using AES-256-GCM
///
/// The encrypted data format is:
/// - 12 bytes nonce
/// - Encrypted data
/// - 16 bytes authentication tag (appended by AES-GCM)
///
/// The final output is base64 encoded.
pub struct ConfigEncryptionService {
    cipher: Aes256Gcm,
}

impl ConfigEncryptionService {
    /// Create a new encryption service with a 32-byte (256-bit) key
    ///
    /// The key should be derived from a secure source (e.g., from configuration secret)
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(key.into());
        Self { cipher }
    }

    /// Create a new encryption service from a base64-encoded key
    pub fn from_base64_key(key: &str) -> CryptoResult<Self> {
        let key_bytes = BASE64
            .decode(key)
            .map_err(|e| CryptoError::Base64Error(e.to_string()))?;

        if key_bytes.len() != 32 {
            return Err(CryptoError::InvalidKey(format!(
                "Key must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        let key_array: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKey("Failed to convert key".to_string()))?;

        Ok(Self::new(&key_array))
    }

    /// Generate a new random 256-bit key
    pub fn generate_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Generate a new random key and return as base64
    pub fn generate_base64_key() -> String {
        let key = Self::generate_key();
        BASE64.encode(key)
    }

    /// Encrypt plaintext data
    ///
    /// Returns base64-encoded ciphertext with embedded nonce
    pub fn encrypt(&self, plaintext: &str) -> CryptoResult<String> {
        // Generate random 12-byte nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the data
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        // Combine nonce + ciphertext
        let mut combined = Vec::with_capacity(12 + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        // Base64 encode
        Ok(BASE64.encode(combined))
    }

    /// Decrypt base64-encoded ciphertext
    ///
    /// The ciphertext must include the nonce prefix
    pub fn decrypt(&self, ciphertext: &str) -> CryptoResult<String> {
        // Base64 decode
        let combined = BASE64
            .decode(ciphertext)
            .map_err(|e| CryptoError::Base64Error(e.to_string()))?;

        if combined.len() < 12 {
            return Err(CryptoError::InvalidData(
                "Ciphertext too short".to_string(),
            ));
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext_bytes) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext_bytes)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| CryptoError::DecryptionFailed(format!("Invalid UTF-8: {}", e)))
    }

    /// Encrypt with a provided data key (for per-configuration encryption)
    ///
    /// Returns (encrypted_content, encrypted_data_key)
    /// The data key is encrypted using the master key
    pub fn encrypt_with_data_key(&self, plaintext: &str) -> CryptoResult<(String, String)> {
        // Generate a random data key
        let data_key = Self::generate_key();

        // Create cipher with data key
        let data_cipher = Aes256Gcm::new((&data_key).into());

        // Encrypt content with data key
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = data_cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        // Combine nonce + ciphertext for content
        let mut combined_content = Vec::with_capacity(12 + ciphertext.len());
        combined_content.extend_from_slice(&nonce_bytes);
        combined_content.extend_from_slice(&ciphertext);
        let encrypted_content = BASE64.encode(combined_content);

        // Encrypt the data key with master key
        let encrypted_data_key = self.encrypt(&BASE64.encode(data_key))?;

        Ok((encrypted_content, encrypted_data_key))
    }

    /// Decrypt using an encrypted data key
    pub fn decrypt_with_data_key(
        &self,
        ciphertext: &str,
        encrypted_data_key: &str,
    ) -> CryptoResult<String> {
        // Decrypt the data key first
        let data_key_base64 = self.decrypt(encrypted_data_key)?;
        let data_key_bytes = BASE64
            .decode(&data_key_base64)
            .map_err(|e| CryptoError::Base64Error(e.to_string()))?;

        if data_key_bytes.len() != 32 {
            return Err(CryptoError::InvalidKey(
                "Invalid data key length".to_string(),
            ));
        }

        let data_key: [u8; 32] = data_key_bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKey("Failed to convert data key".to_string()))?;

        // Create cipher with data key
        let data_cipher = Aes256Gcm::new((&data_key).into());

        // Decode and decrypt content
        let combined = BASE64
            .decode(ciphertext)
            .map_err(|e| CryptoError::Base64Error(e.to_string()))?;

        if combined.len() < 12 {
            return Err(CryptoError::InvalidData(
                "Ciphertext too short".to_string(),
            ));
        }

        let (nonce_bytes, ciphertext_bytes) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = data_cipher
            .decrypt(nonce, ciphertext_bytes)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| CryptoError::DecryptionFailed(format!("Invalid UTF-8: {}", e)))
    }
}

/// Trait for encryption plugins
///
/// Implement this trait to create custom encryption backends
#[async_trait::async_trait]
pub trait EncryptionPlugin: Send + Sync {
    /// Plugin name
    fn name(&self) -> &str;

    /// Encrypt configuration content
    async fn encrypt(&self, plaintext: &str) -> CryptoResult<(String, String)>;

    /// Decrypt configuration content
    async fn decrypt(&self, ciphertext: &str, encrypted_data_key: &str) -> CryptoResult<String>;

    /// Check if encryption is enabled
    fn is_enabled(&self) -> bool;
}

/// Default AES-GCM encryption plugin
pub struct AesGcmEncryptionPlugin {
    service: Option<ConfigEncryptionService>,
    enabled: bool,
}

impl AesGcmEncryptionPlugin {
    /// Create a new disabled plugin (no encryption)
    pub fn disabled() -> Self {
        Self {
            service: None,
            enabled: false,
        }
    }

    /// Create a new enabled plugin with the given key
    pub fn new(key: &str) -> CryptoResult<Self> {
        if key.is_empty() {
            return Ok(Self::disabled());
        }

        let service = ConfigEncryptionService::from_base64_key(key)?;
        Ok(Self {
            service: Some(service),
            enabled: true,
        })
    }
}

#[async_trait::async_trait]
impl EncryptionPlugin for AesGcmEncryptionPlugin {
    fn name(&self) -> &str {
        "aes-gcm"
    }

    async fn encrypt(&self, plaintext: &str) -> CryptoResult<(String, String)> {
        match &self.service {
            Some(service) => service.encrypt_with_data_key(plaintext),
            None => Ok((plaintext.to_string(), String::new())),
        }
    }

    async fn decrypt(&self, ciphertext: &str, encrypted_data_key: &str) -> CryptoResult<String> {
        match &self.service {
            Some(service) => {
                if encrypted_data_key.is_empty() {
                    // Not encrypted
                    Ok(ciphertext.to_string())
                } else {
                    service.decrypt_with_data_key(ciphertext, encrypted_data_key)
                }
            }
            None => Ok(ciphertext.to_string()),
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key() {
        let key = ConfigEncryptionService::generate_base64_key();
        let decoded = BASE64.decode(&key).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = ConfigEncryptionService::generate_key();
        let service = ConfigEncryptionService::new(&key);

        let plaintext = "Hello, World! This is a secret configuration.";
        let encrypted = service.encrypt(plaintext).unwrap();

        // Encrypted should be different from plaintext
        assert_ne!(encrypted, plaintext);

        // Decrypt should return original
        let decrypted = service.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_with_data_key() {
        let master_key = ConfigEncryptionService::generate_key();
        let service = ConfigEncryptionService::new(&master_key);

        let plaintext = "database_password=super_secret_123";
        let (encrypted_content, encrypted_data_key) =
            service.encrypt_with_data_key(plaintext).unwrap();

        // Both should be non-empty
        assert!(!encrypted_content.is_empty());
        assert!(!encrypted_data_key.is_empty());

        // Decrypt should return original
        let decrypted = service
            .decrypt_with_data_key(&encrypted_content, &encrypted_data_key)
            .unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_from_base64_key() {
        let key_base64 = ConfigEncryptionService::generate_base64_key();
        let service = ConfigEncryptionService::from_base64_key(&key_base64).unwrap();

        let plaintext = "Test data";
        let encrypted = service.encrypt(plaintext).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_invalid_key_length() {
        let result = ConfigEncryptionService::from_base64_key("dG9vX3Nob3J0"); // "too_short" in base64
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_ciphertext() {
        let key = ConfigEncryptionService::generate_key();
        let service = ConfigEncryptionService::new(&key);

        let result = service.decrypt("invalid_base64!");
        assert!(result.is_err());

        let result = service.decrypt(&BASE64.encode([0u8; 5])); // Too short
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_encryption_plugin() {
        let key_base64 = ConfigEncryptionService::generate_base64_key();
        let plugin = AesGcmEncryptionPlugin::new(&key_base64).unwrap();

        assert!(plugin.is_enabled());
        assert_eq!(plugin.name(), "aes-gcm");

        let plaintext = "secret_value=12345";
        let (encrypted, data_key) = plugin.encrypt(plaintext).await.unwrap();
        let decrypted = plugin.decrypt(&encrypted, &data_key).await.unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_disabled_plugin() {
        let plugin = AesGcmEncryptionPlugin::disabled();

        assert!(!plugin.is_enabled());

        let plaintext = "not_encrypted";
        let (result, data_key) = plugin.encrypt(plaintext).await.unwrap();
        assert_eq!(result, plaintext);
        assert!(data_key.is_empty());

        let decrypted = plugin.decrypt(plaintext, "").await.unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
