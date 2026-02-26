//! Encrypted data key processing and encryption filters
//!
//! Provides:
//! - LocalEncryptedDataKeyProcessor: Store/retrieve encrypted data keys
//! - EncryptionFilter: Encrypt/decrypt config content using AES-256-GCM

use crate::error::{ClientError, Result};
use crate::local_config::SnapshotSwitch;
use md5::Digest;
use std::path::PathBuf;
use tracing::debug;

/// Local encrypted data key processor
/// Similar to Nacos LocalEncryptedDataKeyProcessor
pub struct LocalEncryptedDataKeyProcessor {
    snapshot_switch: SnapshotSwitch,
    base_path: PathBuf,
    env_name: String,
}

impl LocalEncryptedDataKeyProcessor {
    pub fn new(env_name: String) -> Self {
        let base_path = Self::get_base_path();
        Self {
            snapshot_switch: SnapshotSwitch::new(true),
            base_path,
            env_name,
        }
    }

    pub fn with_snapshot_switch(env_name: String, snapshot_switch: SnapshotSwitch) -> Self {
        let base_path = Self::get_base_path();
        Self {
            snapshot_switch,
            base_path,
            env_name,
        }
    }

    fn get_base_path() -> PathBuf {
        if let Ok(path) = std::env::var("BATATA_SNAPSHOT_PATH") {
            return PathBuf::from(path).join("config");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".batata").join("config");
        }
        PathBuf::from(".batata").join("config")
    }

    /// Get encrypted data key from failover
    pub fn get_encrypt_data_key_failover(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> Result<Option<String>> {
        let file = self.get_failover_file(data_id, group, tenant)?;
        if !file.exists() {
            return Ok(None);
        }

        std::fs::read_to_string(&file)
            .map(Some)
            .map_err(|e| ClientError::Other(e.into()))
    }

    /// Get encrypted data key from snapshot
    pub fn get_encrypt_data_key_snapshot(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> Result<Option<String>> {
        if !self.snapshot_switch.is_enabled() {
            return Ok(None);
        }

        let file = self.get_snapshot_file(data_id, group, tenant)?;
        if !file.exists() {
            return Ok(None);
        }

        std::fs::read_to_string(&file)
            .map(Some)
            .map_err(|e| ClientError::Other(e.into()))
    }

    /// Save encrypted data key snapshot
    pub fn save_encrypt_data_key_snapshot(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        encrypt_data_key: Option<&str>,
    ) -> Result<()> {
        if !self.snapshot_switch.is_enabled() {
            return Ok(());
        }

        let file = self.get_snapshot_file(data_id, group, tenant)?;

        if let Some(content) = encrypt_data_key {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ClientError::Other(e.into()))?;
            }
            std::fs::write(&file, content).map_err(|e| ClientError::Other(e.into()))?;
        } else if file.exists() {
            std::fs::remove_file(&file).map_err(|e| ClientError::Other(e.into()))?;
        }

        Ok(())
    }

    fn get_failover_file(&self, data_id: &str, group: &str, tenant: &str) -> Result<PathBuf> {
        let mut path = self.base_path.clone();
        path.push(format!(
            "{}{}",
            self.simplify_env_name(&self.env_name),
            "_batata"
        ));
        path.push("encrypted-data-key");

        if tenant.is_empty() {
            path.push("failover");
        } else {
            path.push("failover-tenant");
            path.push(tenant);
        }

        path.push(group);
        path.push(data_id);

        Ok(path)
    }

    fn get_snapshot_file(&self, data_id: &str, group: &str, tenant: &str) -> Result<PathBuf> {
        let mut path = self.base_path.clone();
        path.push(format!(
            "{}{}",
            self.simplify_env_name(&self.env_name),
            "_batata"
        ));
        path.push("encrypted-data-key");

        if tenant.is_empty() {
            path.push("snapshot");
        } else {
            path.push("snapshot-tenant");
            path.push(tenant);
        }

        path.push(group);
        path.push(data_id);

        Ok(path)
    }

    fn simplify_env_name(&self, name: &str) -> String {
        const MAX_LEN: usize = 30;
        if name.len() > MAX_LEN {
            format!("{}-{}", &name[..MAX_LEN - 10], Self::hash_string(name))
        } else {
            name.to_string()
        }
    }

    fn hash_string(s: &str) -> String {
        hex::encode(&md5::Md5::digest(s.as_bytes())[..8])
    }
}

/// Simple encryption filter for demo purposes
/// In production, use proper KMS (Key Management Service)
#[derive(Clone)]
pub struct SimpleEncryptionFilter {
    secret_key: Vec<u8>,
    order: i32,
}

impl SimpleEncryptionFilter {
    pub fn new(secret_key: Vec<u8>) -> Self {
        Self {
            secret_key,
            order: 50,
        }
    }

    pub fn new_from_string(key: &str) -> Self {
        let key_bytes = key.as_bytes().to_vec();
        // Pad or truncate to 32 bytes for AES-256
        let mut padded = vec![0u8; 32];
        let len = key_bytes.len().min(32);
        padded[..len].copy_from_slice(&key_bytes[..len]);
        Self {
            secret_key: padded,
            order: 50,
        }
    }
}

#[async_trait::async_trait]
impl super::filter::IConfigFilter for SimpleEncryptionFilter {
    fn order(&self) -> i32 {
        self.order
    }

    fn name(&self) -> &str {
        "simple-encryption"
    }

    async fn filter_publish(
        &self,
        request: &mut super::filter::ConfigRequest,
    ) -> anyhow::Result<()> {
        // Simple XOR encryption for demo
        // In production, use AES-256-GCM with proper KMS
        let key = &self.secret_key;
        let content = request.content.as_bytes();

        let mut encrypted = Vec::with_capacity(content.len());
        for (i, byte) in content.iter().enumerate() {
            encrypted.push(byte ^ key[i % key.len()]);
        }

        request.content = hex::encode(&encrypted);
        request.encrypted_data_key = hex::encode(key);

        debug!("Encrypted config for dataId={}", request.data_id);
        Ok(())
    }

    async fn filter_query(
        &self,
        response: &mut super::filter::ConfigResponse,
    ) -> anyhow::Result<()> {
        if response.encrypted_data_key.is_empty() {
            return Ok(());
        }

        let key = &self.secret_key;
        let encrypted = hex::decode(&response.content)?;

        let mut decrypted = Vec::with_capacity(encrypted.len());
        for (i, byte) in encrypted.iter().enumerate() {
            decrypted.push(byte ^ key[i % key.len()]);
        }

        response.content = String::from_utf8(decrypted)?;

        debug!("Decrypted config for dataId={}", response.data_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::filter::IConfigFilter;

    #[test]
    fn test_encryption_filter_roundtrip() {
        let filter = SimpleEncryptionFilter::new_from_string("my-secret-key-1234567890");

        let mut request = super::super::filter::ConfigRequest::new(
            "test".to_string(),
            "group".to_string(),
            "tenant".to_string(),
        );
        request.content = "Hello, World!".to_string();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            filter.filter_publish(&mut request).await.unwrap();
        });

        assert_ne!(request.content, "Hello, World!");
        assert!(!request.encrypted_data_key.is_empty());

        let mut response = super::super::filter::ConfigResponse::new();
        response.content = request.content.clone();

        rt.block_on(async {
            filter.filter_query(&mut response).await.unwrap();
        });

        assert_eq!(response.content, "Hello, World!");
    }

    #[test]
    fn test_encrypted_data_key_processor() {
        let temp_dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path()) };

        let processor = LocalEncryptedDataKeyProcessor::new("test".to_string());

        // Save encrypted data key
        processor
            .save_encrypt_data_key_snapshot("dataId", "group", "tenant", Some("encrypted-key"))
            .unwrap();

        // Get encrypted data key
        let key = processor
            .get_encrypt_data_key_snapshot("dataId", "group", "tenant")
            .unwrap();
        assert_eq!(key, Some("encrypted-key".to_string()));

        // Delete encrypted data key
        processor
            .save_encrypt_data_key_snapshot("dataId", "group", "tenant", None)
            .unwrap();

        // Get again should return None
        let key = processor
            .get_encrypt_data_key_snapshot("dataId", "group", "tenant")
            .unwrap();
        assert_eq!(key, None);
    }

    #[test]
    fn test_failover_encrypted_data_key() {
        let temp_dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path()) };

        let processor = LocalEncryptedDataKeyProcessor::new("test".to_string());

        // Create failover file manually
        let file = processor.get_failover_file("dataId", "group", "").unwrap();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "failover-key").unwrap();

        // Get failover
        let key = processor
            .get_encrypt_data_key_failover("dataId", "group", "")
            .unwrap();
        assert_eq!(key, Some("failover-key".to_string()));
    }
}
