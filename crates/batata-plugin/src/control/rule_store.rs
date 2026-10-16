//! Rule Storage Implementation (PLG-004)
//!
//! Provides storage backends for control rules with CRUD operations.

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::model::{ConnectionLimitRule, RateLimitRule};

/// Rule storage trait for abstracting storage backends
#[async_trait]
pub trait RuleStore: Send + Sync {
    /// Get a rate limit rule by ID
    async fn get_rate_rule(&self, id: &str) -> anyhow::Result<Option<RateLimitRule>>;

    /// List all rate limit rules
    async fn list_rate_rules(&self) -> anyhow::Result<Vec<RateLimitRule>>;

    /// Create or update a rate limit rule
    async fn save_rate_rule(&self, rule: RateLimitRule) -> anyhow::Result<()>;

    /// Delete a rate limit rule by ID
    async fn delete_rate_rule(&self, id: &str) -> anyhow::Result<bool>;

    /// Get a connection limit rule by ID
    async fn get_connection_rule(&self, id: &str) -> anyhow::Result<Option<ConnectionLimitRule>>;

    /// List all connection limit rules
    async fn list_connection_rules(&self) -> anyhow::Result<Vec<ConnectionLimitRule>>;

    /// Create or update a connection limit rule
    async fn save_connection_rule(&self, rule: ConnectionLimitRule) -> anyhow::Result<()>;

    /// Delete a connection limit rule by ID
    async fn delete_connection_rule(&self, id: &str) -> anyhow::Result<bool>;

    /// Clear all rules
    async fn clear_all(&self) -> anyhow::Result<()>;

    /// Get rule counts
    async fn get_counts(&self) -> anyhow::Result<(usize, usize)>;

    /// Re-read rules from the durable backend into this store.
    ///
    /// Backends that keep everything in memory (pure caches) leave this as a
    /// no-op; persisted backends are expected to override it so a restarted
    /// node recovers its rules instead of starting empty.
    async fn reload_from_disk(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Default file name used when the caller does not supply a rule file path.
///
/// Relative paths resolve against the process working directory, so the
/// server is expected to pass an absolute path derived from its data
/// directory (see `batata.persistence.embedded.data_dir`).
pub const DEFAULT_RULE_FILE: &str = "control_rules.json";

/// In-memory cache layer shared by the persisted backends.
///
/// This is **not** a storage backend: rules held here are lost on restart,
/// which is why Nacos has no equivalent either (its rules live on the local
/// disk and are re-read on startup). It exists purely as the write-through
/// cache in front of the durable store.
#[derive(Debug, Default)]
struct InMemoryRuleCache {
    rate_rules: DashMap<String, RateLimitRule>,
    connection_rules: DashMap<String, ConnectionLimitRule>,
}

impl InMemoryRuleCache {
    /// Create a new, empty cache.
    fn new() -> Self {
        Self {
            rate_rules: DashMap::new(),
            connection_rules: DashMap::new(),
        }
    }

    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

#[async_trait]
impl RuleStore for InMemoryRuleCache {
    async fn get_rate_rule(&self, id: &str) -> anyhow::Result<Option<RateLimitRule>> {
        Ok(self.rate_rules.get(id).map(|r| r.clone()))
    }

    async fn list_rate_rules(&self) -> anyhow::Result<Vec<RateLimitRule>> {
        let mut rules: Vec<_> = self.rate_rules.iter().map(|r| r.clone()).collect();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(rules)
    }

    async fn save_rate_rule(&self, mut rule: RateLimitRule) -> anyhow::Result<()> {
        let now = Self::current_timestamp();
        if rule.created_at == 0 {
            rule.created_at = now;
        }
        rule.updated_at = now;

        if rule.id.is_empty() {
            rule.id = uuid::Uuid::new_v4().to_string();
        }

        self.rate_rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    async fn delete_rate_rule(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.rate_rules.remove(id).is_some())
    }

    async fn get_connection_rule(&self, id: &str) -> anyhow::Result<Option<ConnectionLimitRule>> {
        Ok(self.connection_rules.get(id).map(|r| r.clone()))
    }

    async fn list_connection_rules(&self) -> anyhow::Result<Vec<ConnectionLimitRule>> {
        let mut rules: Vec<_> = self.connection_rules.iter().map(|r| r.clone()).collect();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(rules)
    }

    async fn save_connection_rule(&self, mut rule: ConnectionLimitRule) -> anyhow::Result<()> {
        let now = Self::current_timestamp();
        if rule.created_at == 0 {
            rule.created_at = now;
        }
        rule.updated_at = now;

        if rule.id.is_empty() {
            rule.id = uuid::Uuid::new_v4().to_string();
        }

        self.connection_rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    async fn delete_connection_rule(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.connection_rules.remove(id).is_some())
    }

    async fn clear_all(&self) -> anyhow::Result<()> {
        self.rate_rules.clear();
        self.connection_rules.clear();
        Ok(())
    }

    async fn get_counts(&self) -> anyhow::Result<(usize, usize)> {
        Ok((self.rate_rules.len(), self.connection_rules.len()))
    }
}

/// File-based rule store implementation
///
/// Matches Nacos `LocalDiskRuleStorage`: rules are persisted as JSON on the
/// local disk and served from an in-memory cache. Writes go to the cache
/// first and are then flushed to disk.
pub struct FileRuleStore {
    file_path: String,
    memory_store: InMemoryRuleCache,
}

impl FileRuleStore {
    /// Create a new file-backed rule store persisting to `file_path`.
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            memory_store: InMemoryRuleCache::new(),
        }
    }

    /// Load persisted rules from disk into the in-memory cache.
    ///
    /// Must be called once after construction, otherwise a restarted node
    /// starts with an empty rule set and loses every previously configured
    /// rule on the next write. Safe to call again to pick up rules edited
    /// directly on disk.
    pub async fn load_from_disk(&self) -> anyhow::Result<()> {
        let path = std::path::Path::new(&self.file_path);
        if !path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(path).await?;
        if content.trim().is_empty() {
            return Ok(());
        }
        let data: FileStoreData = serde_json::from_str(&content)?;

        self.memory_store.rate_rules.clear();
        self.memory_store.connection_rules.clear();

        for rule in data.rate_rules {
            self.memory_store.save_rate_rule(rule).await?;
        }

        for rule in data.connection_rules {
            self.memory_store.save_connection_rule(rule).await?;
        }

        Ok(())
    }

    async fn save_to_file(&self) -> anyhow::Result<()> {
        let data = FileStoreData {
            rate_rules: self.memory_store.list_rate_rules().await?,
            connection_rules: self.memory_store.list_connection_rules().await?,
        };

        let content = serde_json::to_string_pretty(&data)?;
        let path = std::path::Path::new(&self.file_path);

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // Write to a sibling temp file and rename it into place: a crash
        // mid-write must not leave a truncated rule file behind.
        let tmp_path = path.with_extension("json.tmp");
        tokio::fs::write(&tmp_path, content).await?;
        tokio::fs::rename(&tmp_path, path).await?;

        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FileStoreData {
    rate_rules: Vec<RateLimitRule>,
    connection_rules: Vec<ConnectionLimitRule>,
}

#[async_trait]
impl RuleStore for FileRuleStore {
    async fn get_rate_rule(&self, id: &str) -> anyhow::Result<Option<RateLimitRule>> {
        self.memory_store.get_rate_rule(id).await
    }

    async fn list_rate_rules(&self) -> anyhow::Result<Vec<RateLimitRule>> {
        self.memory_store.list_rate_rules().await
    }

    async fn save_rate_rule(&self, rule: RateLimitRule) -> anyhow::Result<()> {
        self.memory_store.save_rate_rule(rule).await?;
        self.save_to_file().await
    }

    async fn delete_rate_rule(&self, id: &str) -> anyhow::Result<bool> {
        let deleted = self.memory_store.delete_rate_rule(id).await?;
        if deleted {
            self.save_to_file().await?;
        }
        Ok(deleted)
    }

    async fn get_connection_rule(&self, id: &str) -> anyhow::Result<Option<ConnectionLimitRule>> {
        self.memory_store.get_connection_rule(id).await
    }

    async fn list_connection_rules(&self) -> anyhow::Result<Vec<ConnectionLimitRule>> {
        self.memory_store.list_connection_rules().await
    }

    async fn save_connection_rule(&self, rule: ConnectionLimitRule) -> anyhow::Result<()> {
        self.memory_store.save_connection_rule(rule).await?;
        self.save_to_file().await
    }

    async fn delete_connection_rule(&self, id: &str) -> anyhow::Result<bool> {
        let deleted = self.memory_store.delete_connection_rule(id).await?;
        if deleted {
            self.save_to_file().await?;
        }
        Ok(deleted)
    }

    async fn clear_all(&self) -> anyhow::Result<()> {
        self.memory_store.clear_all().await?;
        self.save_to_file().await
    }

    async fn get_counts(&self) -> anyhow::Result<(usize, usize)> {
        self.memory_store.get_counts().await
    }

    async fn reload_from_disk(&self) -> anyhow::Result<()> {
        self.load_from_disk().await
    }
}

/// Create a rule store based on configuration.
///
/// Local disk is the only built-in backend, matching Nacos where
/// `LocalDiskRuleStorage` is always constructed and an external storage is an
/// optional SPI extension with no built-in implementation. The `Database` and
/// `Raft` variants represent that future extension point and currently fall
/// back to the local disk store with a warning.
pub async fn create_rule_store(
    storage_type: &super::model::RuleStorageType,
    file_path: Option<&str>,
) -> anyhow::Result<Arc<dyn RuleStore>> {
    let path = file_path.unwrap_or(DEFAULT_RULE_FILE);

    match storage_type {
        super::model::RuleStorageType::File => {}
        other => {
            tracing::warn!(
                "Rule storage type {:?} is not implemented, falling back to local disk storage at {}",
                other,
                path
            );
        }
    }

    let store = Arc::new(FileRuleStore::new(path));
    store.load_from_disk().await?;
    Ok(store as Arc<dyn RuleStore>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::model::{RuleMatchType, RuleTargetType};

    fn temp_rule_path(tag: &str) -> String {
        std::env::temp_dir()
            .join(format!(
                "batata-rules-{}-{}.json",
                tag,
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .to_string()
    }

    #[tokio::test]
    async fn test_memory_store_rate_rules() {
        let store = InMemoryRuleCache::new();

        // Create a rule
        let rule = RateLimitRule {
            id: "test-rule-1".to_string(),
            name: "Test Rule".to_string(),
            target_type: RuleTargetType::Ip,
            match_type: RuleMatchType::Exact,
            target_value: "192.168.1.1".to_string(),
            max_tps: 100,
            ..Default::default()
        };

        // Save the rule
        store.save_rate_rule(rule.clone()).await.unwrap();

        // Get the rule
        let retrieved = store.get_rate_rule("test-rule-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Rule");

        // List rules
        let rules = store.list_rate_rules().await.unwrap();
        assert_eq!(rules.len(), 1);

        // Delete the rule
        let deleted = store.delete_rate_rule("test-rule-1").await.unwrap();
        assert!(deleted);

        // Verify deletion
        let retrieved = store.get_rate_rule("test-rule-1").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_memory_store_connection_rules() {
        let store = InMemoryRuleCache::new();

        let rule = ConnectionLimitRule {
            id: "conn-rule-1".to_string(),
            name: "Connection Limit".to_string(),
            max_connections: 1000,
            max_connections_per_ip: 50,
            ..Default::default()
        };

        store.save_connection_rule(rule).await.unwrap();

        let rules = store.list_connection_rules().await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].max_connections, 1000);
    }

    #[tokio::test]
    async fn test_rule_priority_ordering() {
        let store = InMemoryRuleCache::new();

        // Add rules with different priorities
        for (id, priority) in [("low", 1), ("high", 10), ("medium", 5)] {
            let rule = RateLimitRule {
                id: id.to_string(),
                name: id.to_string(),
                priority,
                max_tps: 100,
                ..Default::default()
            };
            store.save_rate_rule(rule).await.unwrap();
        }

        let rules = store.list_rate_rules().await.unwrap();
        assert_eq!(rules[0].name, "high");
        assert_eq!(rules[1].name, "medium");
        assert_eq!(rules[2].name, "low");
    }

    #[tokio::test]
    async fn test_file_store_persists_across_instances() {
        let path = temp_rule_path("roundtrip");

        let store = FileRuleStore::new(path.clone());
        store
            .save_rate_rule(RateLimitRule {
                id: "persisted-1".to_string(),
                name: "Persisted".to_string(),
                target_type: RuleTargetType::Ip,
                match_type: RuleMatchType::Exact,
                target_value: "10.0.0.1".to_string(),
                max_tps: 42,
                ..Default::default()
            })
            .await
            .unwrap();

        // A fresh instance behaves like a restarted node: the rule has to be
        // recovered from disk, not from a live in-memory map.
        let restarted = FileRuleStore::new(path.clone());
        restarted.load_from_disk().await.unwrap();

        let rules = restarted.list_rate_rules().await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "persisted-1");
        assert_eq!(rules[0].max_tps, 42);

        assert!(restarted.delete_rate_rule("persisted-1").await.unwrap());
        let reloaded = FileRuleStore::new(path.clone());
        reloaded.load_from_disk().await.unwrap();
        assert!(reloaded.list_rate_rules().await.unwrap().is_empty());

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_create_rule_store_falls_back_to_local_disk() {
        // Unimplemented backends must still yield a durable local disk store
        // rather than silently degrading to memory-only rules.
        let path = temp_rule_path("fallback");
        let store = create_rule_store(&crate::control::model::RuleStorageType::Raft, Some(&path))
            .await
            .unwrap();

        store
            .save_connection_rule(ConnectionLimitRule {
                id: "conn-1".to_string(),
                name: "Conn".to_string(),
                max_connections: 7,
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(std::path::Path::new(&path).exists());

        let _ = tokio::fs::remove_file(path).await;
    }
}
