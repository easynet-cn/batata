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
}

/// In-memory rule store implementation
#[derive(Debug, Default)]
pub struct MemoryRuleStore {
    rate_rules: DashMap<String, RateLimitRule>,
    connection_rules: DashMap<String, ConnectionLimitRule>,
}

impl MemoryRuleStore {
    pub fn new() -> Self {
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
impl RuleStore for MemoryRuleStore {
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
pub struct FileRuleStore {
    file_path: String,
    memory_store: MemoryRuleStore,
}

impl FileRuleStore {
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            memory_store: MemoryRuleStore::new(),
        }
    }

    #[allow(dead_code)]
    async fn load_from_file(&self) -> anyhow::Result<()> {
        let path = std::path::Path::new(&self.file_path);
        if !path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(path).await?;
        let data: FileStoreData = serde_json::from_str(&content)?;

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
        tokio::fs::write(&self.file_path, content).await?;
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
}

/// Create a rule store based on configuration
pub fn create_rule_store(
    storage_type: &super::model::RuleStorageType,
    file_path: Option<&str>,
) -> Arc<dyn RuleStore> {
    match storage_type {
        super::model::RuleStorageType::Memory => Arc::new(MemoryRuleStore::new()),
        super::model::RuleStorageType::File => {
            let path = file_path.unwrap_or("control_rules.json");
            Arc::new(FileRuleStore::new(path))
        }
        // Database and Raft storage would be implemented in batata-server
        // to avoid circular dependencies
        _ => Arc::new(MemoryRuleStore::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::model::{RuleMatchType, RuleTargetType};

    #[tokio::test]
    async fn test_memory_store_rate_rules() {
        let store = MemoryRuleStore::new();

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
        let store = MemoryRuleStore::new();

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
        let store = MemoryRuleStore::new();

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
}
