//! Local configuration snapshot and failover processor
//!
//! Provides local disaster recovery capabilities:
//! - Snapshot: Cache config content locally for offline access
//! - Failover: Pre-defined local config files for emergency scenarios

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use md5::{Digest, Md5};
use tokio::sync::RwLock;

use crate::error::{ClientError, Result as ClientResult};

type BatataClientError = ClientError;

/// Snapshot switch to enable/disable snapshot functionality
#[derive(Clone)]
pub struct SnapshotSwitch {
    enabled: Arc<AtomicBool>,
}

impl SnapshotSwitch {
    /// Create a snapshot switch with the given initial state.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    /// Create a switch from the GA `configSnapshotEnabled` client property.
    ///
    /// GA `SnapShotSwitch` resolves the property once at class initialization
    /// and defaults to `true`. Processors constructed without a `ClientConfig`
    /// use that same resolution: the GA key (or its `BATATA_` environment
    /// alias) wins when present, otherwise `default` is kept. `BatataClient`
    /// passes its explicit per-client value through [`Self::new`] instead, so
    /// an application-level override is never shadowed by a process variable.
    pub fn from_property(default: bool) -> Self {
        let ga_property =
            std::env::var(crate::client_config::CONFIG_SNAPSHOT_ENABLED_PROPERTY).ok();
        let batata_env =
            std::env::var(crate::client_config::BATATA_CONFIG_SNAPSHOT_ENABLED_ENV).ok();
        let enabled =
            resolve_snapshot_enabled(default, ga_property.as_deref(), batata_env.as_deref());
        Self::new(enabled)
    }

    /// Return whether snapshot functionality is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable snapshot functionality.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

/// Resolve the GA `configSnapshotEnabled` property against an explicit default.
///
/// The GA key wins over the Batata environment alias; an unparsable or absent
/// value keeps `default`. Kept as a pure function so the property precedence
/// can be tested without mutating process-wide environment variables.
pub(crate) fn resolve_snapshot_enabled(
    default: bool,
    ga_property: Option<&str>,
    batata_env: Option<&str>,
) -> bool {
    ga_property
        .or(batata_env)
        .map(|value| crate::client_config::parse_client_property_bool(value, default))
        .unwrap_or(default)
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
            snapshot_switch: SnapshotSwitch::from_property(true),
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

        Self::read_file(&file)
            .map(Some)
            .map_err(BatataClientError::Other)
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
        let file = self.get_snapshot_file(data_id, group, tenant)?;
        Self::write_snapshot_file(self.snapshot_switch.is_enabled(), &file, config)
    }

    /// Async, non-blocking variant of [`Self::save_snapshot`].
    ///
    /// The sync helper does `create_dir_all`/`write`/`remove_file` with
    /// `std::fs`; offload that work to `spawn_blocking` with owned arguments.
    pub async fn save_snapshot_async(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        config: Option<&str>,
    ) -> ClientResult<()> {
        let file = self.get_snapshot_file(data_id, group, tenant)?;
        let enabled = self.snapshot_switch.is_enabled();
        let content = config.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            Self::write_snapshot_file(enabled, &file, content.as_deref())
        })
        .await
        .map_err(|e| {
            BatataClientError::Other(anyhow::anyhow!("snapshot write task failed: {}", e))
        })?
    }

    /// Get failover config for emergency scenarios
    pub fn get_failover(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> ClientResult<Option<String>> {
        let file = self.get_failover_file(data_id, group, tenant)?;
        Self::read_failover_file(&file)
    }

    /// Async, non-blocking variant of [`Self::get_failover`].
    ///
    /// The sync helper reads from disk with `std::fs`; running it on a Tokio
    /// worker would block the executor. Offload the disk IO to
    /// `spawn_blocking` with owned arguments.
    pub async fn get_failover_async(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> ClientResult<Option<String>> {
        let file = self.get_failover_file(data_id, group, tenant)?;
        tokio::task::spawn_blocking(move || Self::read_failover_file(&file))
            .await
            .map_err(|e| {
                BatataClientError::Other(anyhow::anyhow!("failover read task failed: {}", e))
            })?
    }

    /// Clean all snapshots for the current environment
    pub fn clean_all_snapshots(&self) -> ClientResult<()> {
        let env_path = self
            .base_path
            .join(format!("{}{}", self.env_name, "_batata"));
        let snapshot_path = env_path.join("snapshot");

        if snapshot_path.exists() {
            fs::remove_dir_all(&snapshot_path).map_err(|e| BatataClientError::Other(e.into()))?;
        }

        Ok(())
    }

    /// Clean environment-specific snapshots
    pub fn clean_env_snapshot(&self) -> ClientResult<()> {
        let env_path = self
            .base_path
            .join(format!("{}{}", self.env_name, "_batata"));
        let snapshot_path = env_path.join("snapshot");

        if snapshot_path.exists() {
            fs::remove_dir_all(&snapshot_path).map_err(|e| BatataClientError::Other(e.into()))?;
        }

        Ok(())
    }

    /// Get snapshot file path
    fn get_snapshot_file(&self, data_id: &str, group: &str, tenant: &str) -> ClientResult<PathBuf> {
        let mut path = self.base_path.clone();

        // Format: base/{env_name}_batata/snapshot/{tenant?}/{group}/{data_id}
        path.push(format!(
            "{}{}",
            self.simplify_env_name(&self.env_name),
            "_batata"
        ));

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
        path.push(format!(
            "{}{}",
            self.simplify_env_name(&self.server_name),
            "_batata"
        ));
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

    /// Hash string for short ID (8 hex chars)
    fn hash_string(s: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(s.as_bytes());
        let result = hasher.finalize();
        const_hex::encode(&result[..4])
    }

    /// Read file content
    fn read_file(path: &Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {:?}", path))
    }

    /// Read a failover file (sync `std::fs`; called from `spawn_blocking`).
    fn read_failover_file(file: &Path) -> ClientResult<Option<String>> {
        if !file.exists() {
            return Ok(None);
        }
        Self::read_file(file)
            .map(Some)
            .map_err(BatataClientError::Other)
    }

    /// Write/remove a snapshot file (sync `std::fs`; called from
    /// `spawn_blocking`). Owned `file`/`config` keep the blocking closure
    /// `Send + 'static`.
    fn write_snapshot_file(enabled: bool, file: &Path, config: Option<&str>) -> ClientResult<()> {
        if !enabled {
            return Ok(());
        }
        if let Some(content) = config {
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).map_err(|e| BatataClientError::Other(e.into()))?;
            }
            fs::write(file, content).map_err(|e| BatataClientError::Other(e.into()))?;
        } else if file.exists() {
            fs::remove_file(file).map_err(|e| BatataClientError::Other(e.into()))?;
        }
        Ok(())
    }
}

/// Snapshot file metadata
#[derive(Debug, Clone)]
pub struct SnapshotMetadata {
    /// Path of the snapshot file on disk.
    pub file_path: PathBuf,
    /// Size of the snapshot file in bytes.
    pub size: u64,
    /// Last modification time of the snapshot file.
    pub modified: std::time::SystemTime,
    /// MD5 hash of the snapshot content.
    pub md5: String,
}

/// Snapshot manager for batch operations
pub struct SnapshotManager {
    processor: Arc<RwLock<LocalConfigInfoProcessor>>,
}

impl SnapshotManager {
    /// Create a snapshot manager wrapping the given processor.
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
            // The sync `save_snapshot` does `std::fs` IO; offload it so the
            // Tokio worker is never blocked by a large batch.
            processor
                .save_snapshot_async(&data_id, &group, &tenant, Some(&content))
                .await?;
        }
        Ok(())
    }

    /// List all snapshots
    pub async fn list_snapshots(&self) -> ClientResult<Vec<SnapshotMetadata>> {
        // Resolve the path while holding the async lock, then release it before
        // offloading the blocking directory walk to the blocking pool.
        let snapshot_path = {
            let processor = self.processor.read().await;
            processor
                .base_path
                .join(format!("{}{}", processor.env_name, "_batata"))
                .join("snapshot")
        };

        tokio::task::spawn_blocking(move || Self::read_snapshots_dir(&snapshot_path))
            .await
            .map_err(|e| {
                BatataClientError::Other(anyhow::anyhow!("snapshot list task failed: {}", e))
            })?
    }

    /// Synchronous directory walk for [`Self::list_snapshots`]; runs on
    /// `spawn_blocking` so `std::fs` IO never stalls a Tokio worker.
    fn read_snapshots_dir(snapshot_path: &Path) -> ClientResult<Vec<SnapshotMetadata>> {
        let mut snapshots = Vec::new();
        if !snapshot_path.exists() {
            return Ok(snapshots);
        }

        let entries =
            fs::read_dir(snapshot_path).map_err(|e| BatataClientError::Other(e.into()))?;

        for entry in entries {
            let entry = entry.map_err(|e: std::io::Error| BatataClientError::Other(e.into()))?;

            let path = entry.path();
            if path.is_file() {
                let metadata =
                    fs::metadata(&path).map_err(|e| BatataClientError::Other(e.into()))?;

                let content =
                    fs::read_to_string(&path).map_err(|e| BatataClientError::Other(e.into()))?;

                let md5 = const_hex::encode(Md5::digest(content.as_bytes()));

                snapshots.push(SnapshotMetadata {
                    file_path: path,
                    size: metadata.len(),
                    modified: metadata
                        .modified()
                        .unwrap_or_else(|_| std::time::SystemTime::now()),
                    md5,
                });
            }
        }

        Ok(snapshots)
    }
}

/// Serializes tests that touch the `BATATA_SNAPSHOT_PATH` environment variable.
///
/// `LocalConfigInfoProcessor` (and `LocalEncryptedDataKeyProcessor`) read the
/// variable **when they are constructed**, and several tests point it at their
/// own `TempDir`. Cargo runs tests in parallel, so without a lock they
/// overwrite each other's value and fail intermittently — the classic symptom
/// being a snapshot that "was just saved" reading back as `None`.
///
/// Any test that sets the variable, or constructs one of those processors,
/// must hold this lock for its whole body.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquire [`TEST_ENV_LOCK`], tolerating a poisoned mutex (a panicking test
/// must not make every later test panic too).
#[cfg(test)]
pub(crate) fn lock_snapshot_env() -> std::sync::MutexGuard<'static, ()> {
    TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
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
        let _env = lock_snapshot_env();
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
        let file = processor.get_snapshot_file("dataId", "group", "").unwrap();
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
        let _env = lock_snapshot_env();
        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Without tenant
        let file = processor.get_failover_file("dataId", "group", "").unwrap();
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
        let _base_path = temp_dir.path().to_path_buf();

        unsafe {
            // Use custom path via env var
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

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

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Write failover file manually
        let file = processor.get_failover_file("dataId", "group", "").unwrap();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "failover-content").unwrap();

        // Get failover
        let content = processor.get_failover("dataId", "group", "").unwrap();
        assert_eq!(content, Some("failover-content".to_string()));
    }

    #[test]
    fn test_get_failover_not_found() {
        let temp_dir = TempDir::new().unwrap();

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        let content = processor.get_failover("nonexistent", "group", "").unwrap();
        assert_eq!(content, None);
    }

    #[test]
    fn test_snapshot_disabled() {
        let temp_dir = TempDir::new().unwrap();

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let switch = SnapshotSwitch::new(false);
        let processor = LocalConfigInfoProcessor::with_snapshot_switch(
            "test".to_string(),
            "server".to_string(),
            switch,
        );

        // Save should be no-op when disabled
        processor
            .save_snapshot("dataId", "group", "", Some("content"))
            .unwrap();

        // Get should return None when disabled
        let content = processor.get_snapshot("dataId", "group", "").unwrap();
        assert_eq!(content, None);
    }

    #[test]
    fn test_snapshot_toggle() {
        let switch = SnapshotSwitch::new(true);
        assert!(switch.is_enabled());

        switch.set_enabled(false);
        assert!(!switch.is_enabled());

        switch.set_enabled(true);
        assert!(switch.is_enabled());
    }

    #[test]
    fn test_snapshot_switch_clone() {
        let switch = SnapshotSwitch::new(true);
        let cloned = switch.clone();

        // Shared state
        switch.set_enabled(false);
        assert!(!cloned.is_enabled());
    }

    #[test]
    fn test_clean_all_snapshots() {
        let temp_dir = TempDir::new().unwrap();

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Save some snapshots
        processor
            .save_snapshot("id1", "group", "", Some("content1"))
            .unwrap();
        processor
            .save_snapshot("id2", "group", "", Some("content2"))
            .unwrap();

        // Clean all
        processor.clean_all_snapshots().unwrap();

        // Should return None now
        let content = processor.get_snapshot("id1", "group", "").unwrap();
        assert_eq!(content, None);
    }

    #[test]
    fn test_clean_all_snapshots_nonexistent() {
        let temp_dir = TempDir::new().unwrap();

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Should not error when nothing to clean
        processor.clean_all_snapshots().unwrap();
    }

    #[test]
    fn test_clean_env_snapshot() {
        let temp_dir = TempDir::new().unwrap();

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        processor
            .save_snapshot("id1", "group", "", Some("content"))
            .unwrap();

        processor.clean_env_snapshot().unwrap();

        let content = processor.get_snapshot("id1", "group", "").unwrap();
        assert_eq!(content, None);
    }

    #[test]
    fn test_hash_string_consistency() {
        let hash1 = LocalConfigInfoProcessor::hash_string("test-string");
        let hash2 = LocalConfigInfoProcessor::hash_string("test-string");
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 8); // 4 bytes = 8 hex chars
    }

    #[test]
    fn test_hash_string_different_inputs() {
        let hash1 = LocalConfigInfoProcessor::hash_string("input-a");
        let hash2 = LocalConfigInfoProcessor::hash_string("input-b");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_simplify_env_name_short() {
        let _env = lock_snapshot_env();
        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());
        assert_eq!(processor.simplify_env_name("short"), "short");
        assert_eq!(processor.simplify_env_name(""), "");
        assert_eq!(
            processor.simplify_env_name("exactly_thirty_chars_long_xxxx"),
            "exactly_thirty_chars_long_xxxx"
        );
    }

    #[test]
    fn test_simplify_env_name_exactly_30() {
        let _env = lock_snapshot_env();
        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());
        let name = "a".repeat(30);
        let simplified = processor.simplify_env_name(&name);
        assert_eq!(simplified, name); // Exactly 30, no simplification
    }

    #[test]
    fn test_simplify_env_name_over_30() {
        let _env = lock_snapshot_env();
        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());
        let name = "a".repeat(31);
        let simplified = processor.simplify_env_name(&name);
        assert!(simplified.len() <= 30);
        assert!(simplified.contains('-'));
    }

    #[test]
    fn test_snapshot_with_tenant() {
        let temp_dir = TempDir::new().unwrap();

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        // Save with different tenants
        processor
            .save_snapshot("dataId", "group", "tenant-a", Some("content-a"))
            .unwrap();
        processor
            .save_snapshot("dataId", "group", "tenant-b", Some("content-b"))
            .unwrap();

        // Get from different tenants
        let a = processor
            .get_snapshot("dataId", "group", "tenant-a")
            .unwrap();
        let b = processor
            .get_snapshot("dataId", "group", "tenant-b")
            .unwrap();
        assert_eq!(a, Some("content-a".to_string()));
        assert_eq!(b, Some("content-b".to_string()));
    }

    #[test]
    fn test_snapshot_delete_by_none() {
        let temp_dir = TempDir::new().unwrap();

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());

        processor
            .save_snapshot("dataId", "group", "", Some("content"))
            .unwrap();
        assert!(
            processor
                .get_snapshot("dataId", "group", "")
                .unwrap()
                .is_some()
        );

        // Delete by passing None
        processor
            .save_snapshot("dataId", "group", "", None)
            .unwrap();
        assert!(
            processor
                .get_snapshot("dataId", "group", "")
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_snapshot_manager_batch_save() {
        let temp_dir = TempDir::new().unwrap();

        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }

        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());
        // The processor captured the base path above, so the env var no longer
        // matters: release the lock before awaiting (holding a
        // `std::sync::MutexGuard` across `.await` can stall the runtime).
        drop(_env);

        // Save individual snapshot first to verify the path works
        processor
            .save_snapshot("pre-test", "group", "", Some("pre-content"))
            .unwrap();
        assert_eq!(
            processor.get_snapshot("pre-test", "group", "").unwrap(),
            Some("pre-content".to_string())
        );

        let manager = SnapshotManager::new(processor);

        let configs = vec![
            (
                "id1".to_string(),
                "group".to_string(),
                "".to_string(),
                "content1".to_string(),
            ),
            (
                "id2".to_string(),
                "group".to_string(),
                "".to_string(),
                "content2".to_string(),
            ),
        ];

        manager.batch_save_snapshots(configs).await.unwrap();

        // Verify via the manager's own processor
        let processor = manager.processor.read().await;
        assert_eq!(
            processor.get_snapshot("id1", "group", "").unwrap(),
            Some("content1".to_string())
        );
        assert_eq!(
            processor.get_snapshot("id2", "group", "").unwrap(),
            Some("content2".to_string())
        );
    }

    /// The async config helpers must round-trip through `spawn_blocking` while
    /// keeping the sync API intact. Success path: both an async snapshot and an
    /// async failover read return the expected content.
    #[tokio::test]
    async fn test_async_snapshot_and_failover_helpers() {
        let temp_dir = TempDir::new().unwrap();
        let _env = lock_snapshot_env();
        unsafe {
            std::env::set_var("BATATA_SNAPSHOT_PATH", temp_dir.path());
        }
        let processor = LocalConfigInfoProcessor::new("test".to_string(), "server".to_string());
        // The processor captured the base path above; release the env lock
        // before awaiting (holding a std MutexGuard across await stalls the runtime).
        drop(_env);

        processor
            .save_snapshot_async("async-id", "group", "", Some("async-content"))
            .await
            .unwrap();
        assert_eq!(
            processor.get_snapshot("async-id", "group", "").unwrap(),
            Some("async-content".to_string())
        );

        let failover_file = processor
            .get_failover_file("async-id", "group", "")
            .unwrap();
        fs::create_dir_all(failover_file.parent().unwrap()).unwrap();
        fs::write(&failover_file, "async-failover").unwrap();
        assert_eq!(
            processor
                .get_failover_async("async-id", "group", "")
                .await
                .unwrap(),
            Some("async-failover".to_string())
        );
    }

    #[test]
    fn test_resolve_snapshot_enabled() {
        // GA key wins, alias is the fallback, and an invalid value keeps the default.
        assert!(!resolve_snapshot_enabled(true, Some("false"), Some("true")));
        assert!(!resolve_snapshot_enabled(true, None, Some("0")));
        assert!(resolve_snapshot_enabled(true, None, None));
        assert!(!resolve_snapshot_enabled(false, None, None));
        assert!(resolve_snapshot_enabled(true, Some("typo"), None));
        assert!(!resolve_snapshot_enabled(false, Some("typo"), None));
    }

    #[test]
    fn snapshot_processors_resolve_property_switch() {
        // "declaration != call site": both processors must actually construct
        // the switch from the GA property rather than hard-coding `true`.
        const LOCAL: &str = include_str!("local_config.rs");
        const ENCRYPTION: &str = include_str!("config/encryption.rs");
        assert!(LOCAL.contains(concat!("SnapshotSwitch::", "from_property(true)")));
        assert!(ENCRYPTION.contains(concat!("SnapshotSwitch::", "from_property(true)")));
        assert!(LOCAL.contains(concat!("resolve_snapshot_enabled", "(")));
    }
}
