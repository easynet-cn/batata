// RocksDB reader for embedded storage backends
// Provides read-only query operations on the state machine's RocksDB

use std::sync::Arc;

use rocksdb::DB;

use super::state_machine::{
    CF_CONFIG, CF_CONFIG_GRAY, CF_CONFIG_HISTORY, CF_INSTANCES, CF_NAMESPACE, CF_PERMISSIONS,
    CF_ROLES, CF_USERS, RocksStateMachine,
};

/// Read-only interface for querying data stored in RocksDB
///
/// This struct provides read operations on the state machine's RocksDB,
/// used by both `EmbeddedPersistService` (standalone) and
/// `DistributedPersistService` (Raft cluster) backends.
pub struct RocksDbReader {
    db: Arc<DB>,
}

impl RocksDbReader {
    /// Create a new reader wrapping the given RocksDB instance
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    /// Create a reader from a RocksStateMachine
    pub fn from_state_machine(sm: &RocksStateMachine) -> Self {
        Self::new(sm.db())
    }

    /// Get a reference to the underlying RocksDB instance.
    ///
    /// This is useful for operations that need direct DB access,
    /// such as capacity metadata stored in the default column family.
    pub fn db(&self) -> &Arc<DB> {
        &self.db
    }

    // ==================== Config Operations ====================

    /// Get a single config by data_id, group, and tenant
    pub fn get_config(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let key = RocksStateMachine::config_key(data_id, group, tenant);
        self.get_json(CF_CONFIG, &key)
    }

    /// List all configs, optionally filtered by tenant prefix
    pub fn list_configs(&self, tenant: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        if tenant.is_empty() {
            self.iterate_cf_values(CF_CONFIG)
        } else {
            let prefix = format!("{}@@", tenant);
            self.iterate_cf_values_with_prefix(CF_CONFIG, &prefix)
        }
    }

    /// Count configs in a namespace (tenant)
    pub fn count_configs(&self, tenant: &str) -> anyhow::Result<i32> {
        let cf = self.cf_handle(CF_CONFIG)?;
        let mut count = 0i32;

        if tenant.is_empty() {
            let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            for item in iter {
                let _ = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
                count += 1;
            }
        } else {
            let prefix = format!("{}@@", tenant);
            let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
            for item in iter {
                let (key, _) =
                    item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
                let key_str = String::from_utf8_lossy(&key);
                if !key_str.starts_with(&prefix) {
                    break;
                }
                count += 1;
            }
        }

        Ok(count)
    }

    /// Find all distinct group names in a namespace
    pub fn find_all_group_names(&self, tenant: &str) -> anyhow::Result<Vec<String>> {
        let cf = self.cf_handle(CF_CONFIG)?;
        let mut groups = std::collections::HashSet::new();
        let prefix = format!("{}@@", tenant);

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, _) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            // Key format: tenant@@group@@data_id
            let rest = &key_str[prefix.len()..];
            if let Some(sep_pos) = rest.find("@@") {
                groups.insert(rest[..sep_pos].to_string());
            }
        }

        Ok(groups.into_iter().collect())
    }

    /// Search configs with pagination and filters
    ///
    /// Performs in-memory filtering and pagination since RocksDB
    /// doesn't support SQL-like queries.
    #[allow(clippy::too_many_arguments)]
    pub fn search_configs(
        &self,
        namespace_id: &str,
        data_id_pattern: &str,
        group_pattern: &str,
        app_name: &str,
        tags: &[String],
        types: &[String],
        content_pattern: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let configs = self.list_configs(namespace_id)?;

        let filtered: Vec<serde_json::Value> = configs
            .into_iter()
            .filter(|v| {
                // Filter by data_id pattern (prefix match or contains)
                if !data_id_pattern.is_empty() {
                    let did = v["data_id"].as_str().unwrap_or("");
                    if !did.contains(data_id_pattern) {
                        return false;
                    }
                }
                // Filter by group pattern
                if !group_pattern.is_empty() {
                    let grp = v["group"].as_str().unwrap_or("");
                    if !grp.contains(group_pattern) {
                        return false;
                    }
                }
                // Filter by app_name
                if !app_name.is_empty() {
                    let an = v["app_name"].as_str().unwrap_or("");
                    if an != app_name {
                        return false;
                    }
                }
                // Filter by tags
                if !tags.is_empty() {
                    let config_tags = v["config_tags"].as_str().unwrap_or("");
                    for tag in tags {
                        if !config_tags.contains(tag.as_str()) {
                            return false;
                        }
                    }
                }
                // Filter by types
                if !types.is_empty() {
                    let config_type = v["config_type"].as_str().unwrap_or("");
                    if !types.iter().any(|t| t == config_type) {
                        return false;
                    }
                }
                // Filter by content pattern
                if !content_pattern.is_empty() {
                    let ct = v["content"].as_str().unwrap_or("");
                    if !ct.contains(content_pattern) {
                        return false;
                    }
                }
                true
            })
            .collect();

        let total = filtered.len() as u64;
        let offset = (page_no.saturating_sub(1)) * page_size;
        let page_items: Vec<serde_json::Value> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .collect();

        Ok((page_items, total))
    }

    // ==================== Config Gray Operations ====================

    /// Get gray config
    pub fn get_config_gray(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        // Gray configs use a prefix scan since gray_name is part of the key
        let prefix = format!("{}@@{}@@{}@@", tenant, group, data_id);
        let cf = self.cf_handle(CF_CONFIG_GRAY)?;

        let mut iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        if let Some(item) = iter.next() {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if key_str.starts_with(&prefix) {
                let json: serde_json::Value = serde_json::from_slice(&value)?;
                return Ok(Some(json));
            }
        }
        Ok(None)
    }

    // ==================== Config History Operations ====================

    /// Get a config history entry by ID
    pub fn get_config_history_by_id(&self, nid: u64) -> anyhow::Result<Option<serde_json::Value>> {
        // History entries have compound keys; we need to scan and find by id field
        let cf = self.cf_handle(CF_CONFIG_HISTORY)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            if json["id"].as_u64() == Some(nid) || json["id"].as_i64() == Some(nid as i64) {
                return Ok(Some(json));
            }
        }
        Ok(None)
    }

    /// Search config history with pagination
    pub fn search_config_history(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let prefix = format!("{}@@{}@@{}@@", tenant, group, data_id);
        let cf = self.cf_handle(CF_CONFIG_HISTORY)?;

        let mut entries = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());

        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            entries.push(json);
        }

        // Sort by created_time descending (most recent first)
        entries.sort_by(|a, b| {
            let t_b = b["created_time"].as_i64().unwrap_or(0);
            let t_a = a["created_time"].as_i64().unwrap_or(0);
            t_b.cmp(&t_a)
        });

        let total = entries.len() as u64;
        let offset = (page_no.saturating_sub(1)) * page_size;
        let page_items: Vec<serde_json::Value> = entries
            .into_iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .collect();

        Ok((page_items, total))
    }

    // ==================== Namespace Operations ====================

    /// Get a namespace by ID
    pub fn get_namespace(&self, namespace_id: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let key = RocksStateMachine::namespace_key(namespace_id);
        self.get_json(CF_NAMESPACE, &key)
    }

    /// List all namespaces
    pub fn list_namespaces(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.iterate_cf_values(CF_NAMESPACE)
    }

    // ==================== User Operations ====================

    /// Get a user by username
    pub fn get_user(&self, username: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let key = RocksStateMachine::user_key(username);
        self.get_json(CF_USERS, &key)
    }

    /// List all users
    pub fn list_users(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.iterate_cf_values(CF_USERS)
    }

    /// Search users with optional username filter and pagination
    pub fn search_users(
        &self,
        username_pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let all_users = self.list_users()?;

        let filtered: Vec<serde_json::Value> = if username_pattern.is_empty() {
            all_users
        } else {
            all_users
                .into_iter()
                .filter(|v| {
                    let u = v["username"].as_str().unwrap_or("");
                    if accurate {
                        u == username_pattern
                    } else {
                        u.contains(username_pattern)
                    }
                })
                .collect()
        };

        let total = filtered.len() as u64;
        let offset = (page_no.saturating_sub(1)) * page_size;
        let page_items: Vec<serde_json::Value> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .collect();

        Ok((page_items, total))
    }

    // ==================== Role Operations ====================

    /// Get roles by username
    pub fn get_roles_by_username(&self, username: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(CF_ROLES)?;
        let mut roles = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            if json["username"].as_str() == Some(username) {
                roles.push(json);
            }
        }

        Ok(roles)
    }

    /// Search roles with pagination
    pub fn search_roles(
        &self,
        username_pattern: &str,
        role_pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let all_roles = self.iterate_cf_values(CF_ROLES)?;

        let filtered: Vec<serde_json::Value> = all_roles
            .into_iter()
            .filter(|v| {
                let u = v["username"].as_str().unwrap_or("");
                let r = v["role"].as_str().unwrap_or("");

                let username_match = if username_pattern.is_empty() {
                    true
                } else if accurate {
                    u == username_pattern
                } else {
                    u.contains(username_pattern)
                };

                let role_match = if role_pattern.is_empty() {
                    true
                } else if accurate {
                    r == role_pattern
                } else {
                    r.contains(role_pattern)
                };

                username_match && role_match
            })
            .collect();

        let total = filtered.len() as u64;
        let offset = (page_no.saturating_sub(1)) * page_size;
        let page_items: Vec<serde_json::Value> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .collect();

        Ok((page_items, total))
    }

    /// Check if any global admin role exists
    pub fn has_global_admin(&self) -> anyhow::Result<bool> {
        let cf = self.cf_handle(CF_ROLES)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            if json["role"].as_str() == Some("ROLE_ADMIN") {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check if a specific user has the global admin role
    pub fn has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        let key = RocksStateMachine::role_key("ROLE_ADMIN", username);
        let cf = self.cf_handle(CF_ROLES)?;
        Ok(self
            .db
            .get_cf(cf, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB get error: {}", e))?
            .is_some())
    }

    // ==================== Permission Operations ====================

    /// Get a single permission by role, resource, and action
    pub fn get_permission(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let key = RocksStateMachine::permission_key(role, resource, action);
        self.get_json(CF_PERMISSIONS, &key)
    }

    /// Get permissions by role
    pub fn get_permissions_by_role(&self, role: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(CF_PERMISSIONS)?;
        let prefix = format!("{}@@", role);
        let mut permissions = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            permissions.push(json);
        }

        Ok(permissions)
    }

    /// Get permissions by multiple roles
    pub fn get_permissions_by_roles(
        &self,
        roles: &[String],
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let mut permissions = Vec::new();
        for role in roles {
            permissions.extend(self.get_permissions_by_role(role)?);
        }
        Ok(permissions)
    }

    /// Search permissions with pagination
    pub fn search_permissions(
        &self,
        role_pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let all_perms = self.iterate_cf_values(CF_PERMISSIONS)?;

        let filtered: Vec<serde_json::Value> = if role_pattern.is_empty() {
            all_perms
        } else {
            all_perms
                .into_iter()
                .filter(|v| {
                    let r = v["role"].as_str().unwrap_or("");
                    if accurate {
                        r == role_pattern
                    } else {
                        r.contains(role_pattern)
                    }
                })
                .collect()
        };

        let total = filtered.len() as u64;
        let offset = (page_no.saturating_sub(1)) * page_size;
        let page_items: Vec<serde_json::Value> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .collect();

        Ok((page_items, total))
    }

    // ==================== Instance Operations ====================

    /// Get an instance
    pub fn get_instance(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let key =
            RocksStateMachine::instance_key(namespace_id, group_name, service_name, instance_id);
        self.get_json(CF_INSTANCES, &key)
    }

    /// List instances for a service
    pub fn list_instances(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let prefix = format!("{}@@{}@@{}@@", namespace_id, group_name, service_name);
        self.iterate_cf_values_with_prefix(CF_INSTANCES, &prefix)
    }

    // ==================== Internal Helpers ====================

    /// Get a column family handle
    fn cf_handle(&self, cf_name: &str) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(cf_name)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", cf_name))
    }

    /// Get a JSON value from a column family by key
    fn get_json(&self, cf_name: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let cf = self.cf_handle(cf_name)?;
        match self.db.get_cf(cf, key.as_bytes()) {
            Ok(Some(bytes)) => {
                let json: serde_json::Value = serde_json::from_slice(&bytes)?;
                Ok(Some(json))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("RocksDB get error: {}", e)),
        }
    }

    /// Iterate all values in a column family
    fn iterate_cf_values(&self, cf_name: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(cf_name)?;
        let mut values = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            values.push(json);
        }

        Ok(values)
    }

    /// Iterate values in a column family with a key prefix
    fn iterate_cf_values_with_prefix(
        &self,
        cf_name: &str,
        prefix: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(cf_name)?;
        let mut values = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(prefix) {
                break;
            }
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            values.push(json);
        }

        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_db() -> (Arc<DB>, TempDir) {
        let tmp_dir = TempDir::new().unwrap();
        let sm: RocksStateMachine = RocksStateMachine::new(tmp_dir.path()).await.unwrap();
        (sm.db(), tmp_dir)
    }

    #[tokio::test]
    async fn test_reader_config_crud() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        // Write a config directly
        let key = RocksStateMachine::config_key("test-data", "DEFAULT_GROUP", "public");
        let value = serde_json::json!({
            "data_id": "test-data",
            "group": "DEFAULT_GROUP",
            "tenant": "public",
            "content": "key=value",
            "config_type": "properties",
            "app_name": "",
            "md5": "abc123",
            "config_tags": "",
        });
        let cf = db.cf_handle("config").unwrap();
        db.put_cf(cf, key.as_bytes(), value.to_string().as_bytes())
            .unwrap();

        let reader = RocksDbReader::new(db);

        // Test get_config
        let result = reader
            .get_config("test-data", "DEFAULT_GROUP", "public")
            .unwrap();
        assert!(result.is_some());
        let config = result.unwrap();
        assert_eq!(config["data_id"].as_str(), Some("test-data"));
        assert_eq!(config["content"].as_str(), Some("key=value"));

        // Test get_config not found
        let result = reader
            .get_config("nonexistent", "DEFAULT_GROUP", "public")
            .unwrap();
        assert!(result.is_none());

        // Test list_configs
        let configs = reader.list_configs("public").unwrap();
        assert_eq!(configs.len(), 1);

        // Test count_configs
        let count = reader.count_configs("public").unwrap();
        assert_eq!(count, 1);

        let count = reader.count_configs("other").unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_reader_namespace_crud() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        // Write a namespace
        let key = RocksStateMachine::namespace_key("test-ns");
        let value = serde_json::json!({
            "namespace_id": "test-ns",
            "namespace_name": "Test Namespace",
            "namespace_desc": "A test namespace",
        });
        let cf = db.cf_handle("namespace").unwrap();
        db.put_cf(cf, key.as_bytes(), value.to_string().as_bytes())
            .unwrap();

        let reader = RocksDbReader::new(db);

        let result = reader.get_namespace("test-ns").unwrap();
        assert!(result.is_some());
        assert_eq!(
            result.unwrap()["namespace_name"].as_str(),
            Some("Test Namespace")
        );

        let namespaces = reader.list_namespaces().unwrap();
        assert_eq!(namespaces.len(), 1);
    }

    #[tokio::test]
    async fn test_reader_user_search() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        let cf = db.cf_handle("users").unwrap();
        for name in &["alice", "bob", "alice_admin"] {
            let key = RocksStateMachine::user_key(name);
            let value = serde_json::json!({
                "username": name,
                "password_hash": "hash",
                "enabled": true,
            });
            db.put_cf(cf, key.as_bytes(), value.to_string().as_bytes())
                .unwrap();
        }

        let reader = RocksDbReader::new(db);

        // Search with pattern
        let (users, total) = reader.search_users("alice", false, 1, 10).unwrap();
        assert_eq!(total, 2); // alice and alice_admin
        assert_eq!(users.len(), 2);

        // Accurate search
        let (users, total) = reader.search_users("alice", true, 1, 10).unwrap();
        assert_eq!(total, 1);
        assert_eq!(users.len(), 1);

        // All users
        let (users, total) = reader.search_users("", false, 1, 10).unwrap();
        assert_eq!(total, 3);
        assert_eq!(users.len(), 3);
    }

    #[tokio::test]
    async fn test_reader_roles_and_permissions() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        // Write roles
        let role_cf = db.cf_handle("roles").unwrap();
        let key1 = RocksStateMachine::role_key("ROLE_ADMIN", "admin");
        let val1 = serde_json::json!({"role": "ROLE_ADMIN", "username": "admin"});
        db.put_cf(role_cf, key1.as_bytes(), val1.to_string().as_bytes())
            .unwrap();

        let key2 = RocksStateMachine::role_key("ROLE_USER", "alice");
        let val2 = serde_json::json!({"role": "ROLE_USER", "username": "alice"});
        db.put_cf(role_cf, key2.as_bytes(), val2.to_string().as_bytes())
            .unwrap();

        // Write permissions
        let perm_cf = db.cf_handle("permissions").unwrap();
        let pkey = RocksStateMachine::permission_key("ROLE_ADMIN", "public::*", "rw");
        let pval =
            serde_json::json!({"role": "ROLE_ADMIN", "resource": "public::*", "action": "rw"});
        db.put_cf(perm_cf, pkey.as_bytes(), pval.to_string().as_bytes())
            .unwrap();

        let reader = RocksDbReader::new(db);

        // Test roles
        let roles = reader.get_roles_by_username("admin").unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0]["role"].as_str(), Some("ROLE_ADMIN"));

        // Test has_global_admin
        assert!(reader.has_global_admin().unwrap());
        assert!(reader.has_global_admin_by_username("admin").unwrap());
        assert!(!reader.has_global_admin_by_username("alice").unwrap());

        // Test permissions
        let perms = reader.get_permissions_by_role("ROLE_ADMIN").unwrap();
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0]["resource"].as_str(), Some("public::*"));

        // Test permissions by roles
        let perms = reader
            .get_permissions_by_roles(&["ROLE_ADMIN".to_string(), "ROLE_USER".to_string()])
            .unwrap();
        assert_eq!(perms.len(), 1); // Only ROLE_ADMIN has permissions
    }

    #[tokio::test]
    async fn test_reader_group_names() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        let cf = db.cf_handle("config").unwrap();
        for (data_id, group) in &[("d1", "GROUP_A"), ("d2", "GROUP_A"), ("d3", "GROUP_B")] {
            let key = RocksStateMachine::config_key(data_id, group, "public");
            let value = serde_json::json!({
                "data_id": data_id,
                "group": group,
                "tenant": "public",
                "content": "test",
            });
            db.put_cf(cf, key.as_bytes(), value.to_string().as_bytes())
                .unwrap();
        }

        let reader = RocksDbReader::new(db);
        let mut groups = reader.find_all_group_names("public").unwrap();
        groups.sort();
        assert_eq!(groups, vec!["GROUP_A", "GROUP_B"]);
    }
}
