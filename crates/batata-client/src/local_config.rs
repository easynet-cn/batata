//! Local configuration snapshot and failover processor
//!
//! Provides local disaster recovery capabilities:
//! - Snapshot: Cache config content locally for offline access
//! - Failover: Pre-defined local config files for emergency scenarios

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use md5::{Md5, Digest};
use tokio::sync::RwLock;

use crate::error::{ClientError, Result as ClientResult};

type BatataClientError = ClientError;

/// Snapshot switch to enable/disable snapshot functionality
#[derive(Clone)]
pub struct SnapshotSwitch {
    enabled: Arc<AtomicBool>,
}

impl SnapshotSwitch {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

/// Local config info processor for snapshot and failover
pub struct LocalConfigInfoProcessor {
    snapshot_switch: SnapshotSwitch,
    base_path: PathBuf,
    env_name: String,
    server_name: String,
}

impl LocalConfigInfoProcessor {
    /// Create a new local config info processor
    pub fn new(env_name: String, server_name: String) -> Self {
        let base_path = Self::get_local_snapshot_path();
        Self {
            snapshot_switch: SnapshotSwitch::new(true),
            base_path,
            env_name,
            server_name,
        }
    }

    /// Create with custom snapshot switch
    pub fn with_snapshot_switch(
        env_name: String,
        server_name: String,
        snapshot_switch: SnapshotSwitch,
    ) -> Self {
        let base_path = Self::get_local_snapshot_path();
        Self {
            snapshot_switch,
            base_path,
            env_name,
            server_name,
        }
    }

    /// Get local snapshot path from environment or default
    fn get_local_snapshot_path() -> PathBuf {
        if let Ok(path) = std::env::var("BATATA_SNAPSHOT_PATH") {
            return PathBuf::from(path).join("config");
        }

        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".batata").join("config");
        }

        PathBuf::from(".batata").join("config")
    }

    /// Get snapshot for a config
    ///
    /// Returns None if snapshot is disabled or file doesn't exist
    pub fn get_snapshot(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> ClientResult<Option<String>> {
        if !self.snapshot_switch.is_enabled() {
            return Ok(None);
        }

        let file = self.get_snapshot_file(data_id, group, tenant)?;
        if !file.exists() {
            return Ok(None);
        }

        Self::read_file(&file).map(Some).map_err(|e| {
            BatataClientError::Other(e.into())
        })
    }

    /// Save snapshot for a config
    ///
    /// If config is None, delete the snapshot file
    pub fn save_snapshot(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        config: Option<&str>,
    ) -> ClientResult<()> {
        if !self.snapshot_switch.is_enabled() {
            return Ok(());
        }

        let file = self.get_snapshot_file(data_id, group, tenant)?;

        if let Some(content) = config {
            // Create parent directories
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    BatataClientError::Other(e.into())
                })?;
            }

            // Write content
            fs::write(&file, content).map_err(|e| {
                BatataClientError::Other(e.into())
            })?;
        } else {
            // Delete snapshot
            if file.exists() {
                fs::remove_file(&file).map_err(|e| {
                    BatataClientError::Other(e.into())
                })?;
            }
        }

        Ok(())
    }

    /// Get failover config for emergency scenarios
    pub fn get_failover(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> ClientResult<Option<String>> {
        let file = self.get_failover_file(data_id, group, tenant)?;
        if !file.exists() {
            return Ok(None);
        }

        Self::read_file(&file).map(Some).map_err(|e| {
            BatataClientError::Other(e.into())
        })
    }

    /// Clean all snapshots for the current environment
    pub fn clean_all_snapshots(&self) -> ClientResult<()> {
        let env_path = self.base_path.join(format!("{}{}", self.env_name, "_batata"));
        let snapshot_path = env_path.join("snapshot");

        if snapshot_path.exists() {
            fs::remove_dir_all(&snapshot_path).map_err(|e| {
                BatataClientError::Other(e.into())
            })?;
        }

        Ok(())
    }

    /// Clean environment-specific snapshots
    pub fn clean_env_snapshot(&self) -> ClientResult<()> {
        let env_path = self.base_path.join(format!("{}{}", self.env_name, "_batata"));
        let snapshot_path = env_path.join("snapshot");

        if snapshot_path.exists() {
            fs::remove_dir_all(&snapshot_path).map_err(|e| {
                BatataClientError::Other(e.into())
            })?;
        }

        Ok(())
    }

    /// Get snapshot file path
    fn get_snapshot_file(&self, data_id: &str, group: &str, tenant: &str) -> ClientResult<PathBuf> {
        let mut path = self.base_path.clone();

        // Format: base/{env_name}_batata/snapshot/{tenant?}/{group}/{data_id}
        path.push(format!("{}{}", self.simplify_env_name(&self.env_name), "_batata"));

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

    /// Get failover file path
    fn get_failover_file(&self, data_id: &str, group: &str, tenant: &str) -> ClientResult<PathBuf> {
        let mut path = self.base_path.clone();

        // Format: base/{server_name}_batata/data/config-data[-tenant]/{tenant?}/{group}/{data_id}
        path.push(format!("{}{}", self.simplify_env_name(&self.server_name), "_batata"));
        path.push("data");

        if tenant.is_empty() {
            path.push("config-data");
        } else {
            path.push("config-data-tenant");
            path.push(tenant);
        }

        path.push(group);
        path.push(data_id);

        Ok(path)
    }

    /// Simplify environment name if over limit (max 30 chars)
    fn simplify_env_name(&self, name: &str) -> String {
        const MAX_LEN: usize = 30;
        if name.len() > MAX_LEN {
            format!("{}-{}", &name[..MAX_LEN - 10], Self::hash_string(name))
        } else {
            name.to_string()
        }
    }

    /// Hash string for short ID
    fn hash_string(s: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(s.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// Read file content
    fn read_file(path: &Path) -> Result<String> {
        fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {:?}", path))
    }
}

/// Snapshot file metadata
#[derive(Debug, Clone)]
pub struct SnapshotMetadata {
    pub file_path: PathBuf,
    pub size: u64,
    pub modified: std::time::SystemTime,
    pub md5: String,
}

/// Snapshot manager for batch operations
pub struct SnapshotManager {
    processor: Arc<RwLock<LocalConfigInfoProcessor>>,
}

impl SnapshotManager {
    pub fn new(processor: LocalConfigInfoProcessor) -> Self {
        Self {
            processor: Arc::new(RwLock::new(processor)),
        }
    }

    /// Batch save snapshots
    pub async fn batch_save_snapshots(
        &self,
        configs: Vec<(String, String, String, String)>,
    ) -> ClientResult<()> {
        let processor = self.processor.read().await;
        for (data_id, group, tenant, content) in configs {
            processor.save_snapshot(&data_id, &group, &tenant, Some(&content))?;
        }
        Ok(())
    }

    /// List all snapshots
    pub async fn list_snapshots(&self) -> ClientResult<Vec<SnapshotMetadata>> {
        let processor = self.processor.read().await;
        let mut snapshots = Vec::new();

        // Walk snapshot directory
        let env_path = processor.base_path.join(format!("{}{}", processor.env_name, "_batata"));
        let snapshot_path = env_path.join("snapshot");

        if !snapshot_path.exists() {
            return Ok(snapshots);
        }

        let entries = fs::read_dir(&snapshot_path).map_err(|e| {
            BatataClientError::Other(e.into())
        })?;

        for entry in entries {
            let entry = entry.map_err(|e: std::io::Error| {
                BatataClientError::Other(e.into())
            })?;

            let path = entry.path();
            if path.is_file() {
                let metadata = fs::metadata(&path).map_err(|e| {
                    BatataClientError::Other(e.into())
                })?;

                let content = fs::read_to_string(&path).map_err(|e| {
                    BatataClientError::Other(e.into())
                })?;

                let md5 = format!("{:x}", Md5::digest(content.as_bytes()));

                snapshots.push(SnapshotMetadata {
                    file_path: path,
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or_else(|_| std::time::SystemTime::now()),
                    md5,
                });
            }
        }

        Ok(snapshots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_snapshot_switch() {
        let switch = SnapshotSwitch::new(true);
        assert!(switch.is_enabled());
        switch.set_enabled(false);
        assert!(!switch.is_enabled());
    }

    #[test]
    fn test_simplify_env_name() {
        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Short name
        assert_eq!(processor.simplify_env_name("short"), "short");

        // Long name should be simplified
        let long_name = "a".repeat(40);
        let simplified = processor.simplify_env_name(&long_name);
        assert!(simplified.len() <= 30);
    }

    #[test]
    fn test_get_snapshot_file() {
        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Without tenant
        let file = processor
            .get_snapshot_file("dataId", "group", "")
            .unwrap();
        assert!(file.to_str().unwrap().contains("snapshot"));
        assert!(file.to_str().unwrap().ends_with("group/dataId"));

        // With tenant
        let file = processor
            .get_snapshot_file("dataId", "group", "tenant")
            .unwrap();
        assert!(file.to_str().unwrap().contains("snapshot-tenant/tenant"));
        assert!(file.to_str().unwrap().ends_with("group/dataId"));
    }

    #[test]
    fn test_get_failover_file() {
        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Without tenant
        let file = processor
            .get_failover_file("dataId", "group", "")
            .unwrap();
        assert!(file.to_str().unwrap().contains("config-data"));

        // With tenant
        let file = processor
            .get_failover_file("dataId", "group", "tenant")
            .unwrap();
        assert!(file.to_str().unwrap().contains("config-data-tenant"));
    }

    #[test]
    fn test_snapshot_save_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        // Use custom path via env var
        std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Save snapshot
        processor
            .save_snapshot("dataId", "group", "tenant", Some("content"))
            .unwrap();

        // Get snapshot
        let content = processor.get_snapshot("dataId", "group", "tenant").unwrap();
        assert_eq!(content, Some("content".to_string()));

        // Delete snapshot
        processor
            .save_snapshot("dataId", "group", "tenant", None)
            .unwrap();

        // Get again should return None
        let content = processor.get_snapshot("dataId", "group", "tenant").unwrap();
        assert_eq!(content, None);
    }

    #[test]
    fn test_failover_save_and_get() {
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Write failover file manually
        let file = processor
            .get_failover_file("dataId", "group", "")
            .unwrap();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "failover-content").unwrap();

        // Get failover
        let content = processor.get_failover("dataId", "group", "").unwrap();
        assert_eq!(content, Some("failover-content".to_string()));
    }
}
