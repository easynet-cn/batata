// ACL Storage Backend
//
// Provides a unified storage abstraction for ACL data (tokens, policies, roles,
// auth methods, binding rules). Two backends:
// - Memory: instance-level DashMaps (for tests and standalone mode)
// - Persistent: RocksDB as single source of truth (for production/cluster mode)

use std::sync::Arc;

use dashmap::DashMap;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::acl::{AclPolicy, AclRole, AclToken, AuthMethod, BindingRule};
use crate::constants::CF_CONSUL_ACL;

/// Storage backend for ACL data.
///
/// When `Persistent`, RocksDB is the single source of truth.
/// When `Memory`, instance-level DashMaps are used (no global state).
#[derive(Clone)]
#[allow(clippy::collapsible_if)]
pub enum AclStore {
    /// In-memory storage (for tests and standalone mode without persistence)
    Memory {
        tokens: Arc<DashMap<String, AclToken>>,
        policies: Arc<DashMap<String, AclPolicy>>,
        roles: Arc<DashMap<String, AclRole>>,
        auth_methods: Arc<DashMap<String, AuthMethod>>,
        binding_rules: Arc<DashMap<String, BindingRule>>,
    },
    /// RocksDB as single source of truth (for persistent/cluster mode)
    Persistent { db: Arc<DB> },
}

impl AclStore {
    /// Create an in-memory store
    pub fn memory() -> Self {
        Self::Memory {
            tokens: Arc::new(DashMap::new()),
            policies: Arc::new(DashMap::new()),
            roles: Arc::new(DashMap::new()),
            auth_methods: Arc::new(DashMap::new()),
            binding_rules: Arc::new(DashMap::new()),
        }
    }

    /// Create a persistent store backed by RocksDB
    pub fn persistent(db: Arc<DB>) -> Self {
        Self::Persistent { db }
    }

    // ========================================================================
    // Generic RocksDB helpers
    // ========================================================================

    fn rocks_db(&self) -> Option<&Arc<DB>> {
        match self {
            Self::Persistent { db } => Some(db),
            Self::Memory { .. } => None,
        }
    }

    fn rocks_get_raw(&self, key: &str) -> Option<Vec<u8>> {
        let db = self.rocks_db()?;
        let cf = db.cf_handle(CF_CONSUL_ACL)?;
        db.get_cf(cf, key.as_bytes()).ok().flatten()
    }

    fn rocks_get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        let bytes = self.rocks_get_raw(key)?;
        serde_json::from_slice(&bytes).ok()
    }

    fn rocks_put<T: Serialize>(&self, key: &str, value: &T) {
        if let Some(db) = self.rocks_db()
            && let Some(cf) = db.cf_handle(CF_CONSUL_ACL)
        {
            match serde_json::to_vec(value) {
                Ok(bytes) => {
                    if let Err(e) = db.put_cf(cf, key.as_bytes(), &bytes) {
                        error!("Failed to persist ACL '{}': {}", key, e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize ACL '{}': {}", key, e);
                }
            }
        }
    }

    fn rocks_put_raw(&self, key: &str, value: &[u8]) {
        if let Some(db) = self.rocks_db()
            && let Some(cf) = db.cf_handle(CF_CONSUL_ACL)
        {
            if let Err(e) = db.put_cf(cf, key.as_bytes(), value) {
                error!("Failed to persist ACL '{}': {}", key, e);
            }
        }
    }

    fn rocks_delete(&self, key: &str) {
        if let Some(db) = self.rocks_db()
            && let Some(cf) = db.cf_handle(CF_CONSUL_ACL)
        {
            if let Err(e) = db.delete_cf(cf, key.as_bytes()) {
                error!("Failed to delete ACL '{}': {}", key, e);
            }
        }
    }

    /// List all values with a given key prefix, deserializing each as T.
    fn rocks_list<T: for<'de> Deserialize<'de>>(&self, prefix: &str) -> Vec<T> {
        let db = match self.rocks_db() {
            Some(db) => db,
            None => return vec![],
        };
        let cf = match db.cf_handle(CF_CONSUL_ACL) {
            Some(cf) => cf,
            None => return vec![],
        };

        let prefix_bytes = prefix.as_bytes();
        let mut results = Vec::new();
        let iter = db.iterator_cf(
            cf,
            rocksdb::IteratorMode::From(prefix_bytes, rocksdb::Direction::Forward),
        );
        for item in iter.flatten() {
            let (key_bytes, value_bytes) = item;
            if !key_bytes.starts_with(prefix_bytes) {
                break;
            }
            if let Ok(val) = serde_json::from_slice::<T>(&value_bytes) {
                results.push(val);
            }
        }
        results
    }

    // ========================================================================
    // Token operations
    //
    // Keys: token::<secret_id> → JSON
    // Index: token_accessor::<accessor_id> → <secret_id>
    // ========================================================================

    pub fn get_token(&self, secret_id: &str) -> Option<AclToken> {
        match self {
            Self::Memory { tokens, .. } => tokens.get(secret_id).map(|e| e.value().clone()),
            Self::Persistent { .. } => self.rocks_get(&format!("token::{}", secret_id)),
        }
    }

    pub fn put_token(&self, secret_id: &str, token: &AclToken) {
        match self {
            Self::Memory { tokens, .. } => {
                tokens.insert(secret_id.to_string(), token.clone());
            }
            Self::Persistent { .. } => {
                self.rocks_put(&format!("token::{}", secret_id), token);
                // Maintain accessor → secret_id index
                self.rocks_put_raw(
                    &format!("token_accessor::{}", token.accessor_id),
                    secret_id.as_bytes(),
                );
            }
        }
    }

    pub fn delete_token_by_secret(&self, secret_id: &str) -> Option<AclToken> {
        match self {
            Self::Memory { tokens, .. } => tokens.remove(secret_id).map(|(_, t)| t),
            Self::Persistent { .. } => {
                let token: AclToken = self.rocks_get(&format!("token::{}", secret_id))?;
                self.rocks_delete(&format!("token::{}", secret_id));
                self.rocks_delete(&format!("token_accessor::{}", token.accessor_id));
                Some(token)
            }
        }
    }

    /// Find token by accessor_id. Returns (secret_id, token).
    pub fn find_token_by_accessor(&self, accessor_id: &str) -> Option<(String, AclToken)> {
        match self {
            Self::Memory { tokens, .. } => tokens.iter().find_map(|entry| {
                if entry.value().accessor_id == accessor_id {
                    Some((entry.key().clone(), entry.value().clone()))
                } else {
                    None
                }
            }),
            Self::Persistent { .. } => {
                let secret_id = String::from_utf8(
                    self.rocks_get_raw(&format!("token_accessor::{}", accessor_id))?,
                )
                .ok()?;
                let token = self.rocks_get(&format!("token::{}", secret_id))?;
                Some((secret_id, token))
            }
        }
    }

    pub fn list_tokens(&self) -> Vec<AclToken> {
        match self {
            Self::Memory { tokens, .. } => tokens.iter().map(|e| e.value().clone()).collect(),
            Self::Persistent { .. } => self.rocks_list("token::"),
        }
    }

    /// Delete tokens matching a predicate. Returns removed (secret_id, token) pairs.
    pub fn retain_tokens<F: Fn(&str, &AclToken) -> bool>(
        &self,
        keep: F,
    ) -> Vec<(String, AclToken)> {
        match self {
            Self::Memory { tokens, .. } => {
                let mut removed = Vec::new();
                tokens.retain(|secret_id, token| {
                    if keep(secret_id, token) {
                        true
                    } else {
                        removed.push((secret_id.clone(), token.clone()));
                        false
                    }
                });
                removed
            }
            Self::Persistent { .. } => {
                let all = self.list_tokens();
                let mut removed = Vec::new();
                for token in all {
                    let secret_id = token.secret_id.clone().unwrap_or_default();
                    if !keep(&secret_id, &token) {
                        self.rocks_delete(&format!("token::{}", secret_id));
                        self.rocks_delete(&format!("token_accessor::{}", token.accessor_id));
                        removed.push((secret_id, token));
                    }
                }
                removed
            }
        }
    }

    /// Check if a token with the given accessor_id exists (O(1) via index).
    pub fn has_token_by_accessor(&self, accessor_id: &str) -> bool {
        match self {
            Self::Memory { tokens, .. } => {
                tokens.iter().any(|e| e.value().accessor_id == accessor_id)
            }
            Self::Persistent { .. } => self
                .rocks_get_raw(&format!("token_accessor::{}", accessor_id))
                .is_some(),
        }
    }

    /// Find token by accessor_id and return it (O(1) via index).
    pub fn get_token_by_accessor(&self, accessor_id: &str) -> Option<AclToken> {
        self.find_token_by_accessor(accessor_id)
            .map(|(_, token)| token)
    }

    /// Check if any token matches a predicate.
    /// NOTE: For accessor_id checks, prefer `has_token_by_accessor()` (O(1) vs O(N)).
    pub fn any_token<F: Fn(&AclToken) -> bool>(&self, f: F) -> bool {
        match self {
            Self::Memory { tokens, .. } => tokens.iter().any(|e| f(e.value())),
            Self::Persistent { .. } => self.list_tokens().iter().any(&f),
        }
    }

    /// Find first token matching a predicate.
    /// NOTE: For accessor_id lookups, prefer `get_token_by_accessor()` (O(1) vs O(N)).
    pub fn find_token<F: Fn(&AclToken) -> bool>(&self, f: F) -> Option<AclToken> {
        match self {
            Self::Memory { tokens, .. } => tokens.iter().find_map(|e| {
                if f(e.value()) {
                    Some(e.value().clone())
                } else {
                    None
                }
            }),
            Self::Persistent { .. } => self.list_tokens().into_iter().find(|t| f(t)),
        }
    }

    // ========================================================================
    // Policy operations
    //
    // Keys: policy::<id> → JSON
    // Index: policy_name::<name> → <id>
    // ========================================================================

    pub fn get_policy(&self, id_or_name: &str) -> Option<AclPolicy> {
        match self {
            Self::Memory { policies, .. } => policies.get(id_or_name).map(|e| e.value().clone()),
            Self::Persistent { .. } => {
                // Try by ID first
                if let Some(p) = self.rocks_get::<AclPolicy>(&format!("policy::{}", id_or_name)) {
                    return Some(p);
                }
                // Try by name (via secondary index)
                let id =
                    String::from_utf8(self.rocks_get_raw(&format!("policy_name::{}", id_or_name))?)
                        .ok()?;
                self.rocks_get(&format!("policy::{}", id))
            }
        }
    }

    pub fn put_policy(&self, policy: &AclPolicy) {
        match self {
            Self::Memory { policies, .. } => {
                policies.insert(policy.id.clone(), policy.clone());
                policies.insert(policy.name.clone(), policy.clone());
            }
            Self::Persistent { .. } => {
                self.rocks_put(&format!("policy::{}", policy.id), policy);
                self.rocks_put_raw(
                    &format!("policy_name::{}", policy.name),
                    policy.id.as_bytes(),
                );
            }
        }
    }

    /// Remove a policy by ID. Returns the removed policy if found.
    pub fn remove_policy(&self, id: &str) -> Option<AclPolicy> {
        match self {
            Self::Memory { policies, .. } => {
                let (_, policy) = policies.remove(id)?;
                policies.remove(&policy.name);
                Some(policy)
            }
            Self::Persistent { .. } => {
                let policy: AclPolicy = self.rocks_get(&format!("policy::{}", id))?;
                self.rocks_delete(&format!("policy::{}", id));
                self.rocks_delete(&format!("policy_name::{}", policy.name));
                Some(policy)
            }
        }
    }

    /// Remove the name index for an old name (used when renaming a policy).
    pub fn remove_policy_name_index(&self, name: &str) {
        match self {
            Self::Memory { policies, .. } => {
                policies.remove(name);
            }
            Self::Persistent { .. } => {
                self.rocks_delete(&format!("policy_name::{}", name));
            }
        }
    }

    pub fn policy_name_exists(&self, name: &str) -> bool {
        match self {
            Self::Memory { policies, .. } => policies.contains_key(name),
            Self::Persistent { .. } => self
                .rocks_get_raw(&format!("policy_name::{}", name))
                .is_some(),
        }
    }

    pub fn list_policies(&self) -> Vec<AclPolicy> {
        match self {
            Self::Memory { policies, .. } => {
                let mut seen = std::collections::HashSet::new();
                policies
                    .iter()
                    .filter_map(|entry| {
                        let policy = entry.value();
                        if seen.insert(policy.id.clone()) {
                            Some(policy.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            Self::Persistent { .. } => self.rocks_list("policy::"),
        }
    }

    // ========================================================================
    // Role operations
    //
    // Keys: role::<id> → JSON
    // Index: role_name::<name> → <id>
    // ========================================================================

    pub fn get_role(&self, id_or_name: &str) -> Option<AclRole> {
        match self {
            Self::Memory { roles, .. } => roles.get(id_or_name).map(|e| e.value().clone()),
            Self::Persistent { .. } => {
                if let Some(r) = self.rocks_get::<AclRole>(&format!("role::{}", id_or_name)) {
                    return Some(r);
                }
                let id =
                    String::from_utf8(self.rocks_get_raw(&format!("role_name::{}", id_or_name))?)
                        .ok()?;
                self.rocks_get(&format!("role::{}", id))
            }
        }
    }

    pub fn put_role(&self, role: &AclRole) {
        match self {
            Self::Memory { roles, .. } => {
                roles.insert(role.id.clone(), role.clone());
                roles.insert(role.name.clone(), role.clone());
            }
            Self::Persistent { .. } => {
                self.rocks_put(&format!("role::{}", role.id), role);
                self.rocks_put_raw(&format!("role_name::{}", role.name), role.id.as_bytes());
            }
        }
    }

    /// Remove a role by ID. Returns the removed role if found.
    pub fn remove_role(&self, id: &str) -> Option<AclRole> {
        match self {
            Self::Memory { roles, .. } => {
                let (_, role) = roles.remove(id)?;
                roles.remove(&role.name);
                Some(role)
            }
            Self::Persistent { .. } => {
                let role: AclRole = self.rocks_get(&format!("role::{}", id))?;
                self.rocks_delete(&format!("role::{}", id));
                self.rocks_delete(&format!("role_name::{}", role.name));
                Some(role)
            }
        }
    }

    /// Remove the name index for an old name (used when renaming a role).
    pub fn remove_role_name_index(&self, name: &str) {
        match self {
            Self::Memory { roles, .. } => {
                roles.remove(name);
            }
            Self::Persistent { .. } => {
                self.rocks_delete(&format!("role_name::{}", name));
            }
        }
    }

    pub fn list_roles(&self) -> Vec<AclRole> {
        match self {
            Self::Memory { roles, .. } => {
                let mut seen = std::collections::HashSet::new();
                roles
                    .iter()
                    .filter_map(|entry| {
                        let role = entry.value();
                        if seen.insert(role.id.clone()) {
                            Some(role.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            Self::Persistent { .. } => self.rocks_list("role::"),
        }
    }

    // ========================================================================
    // Auth method operations
    //
    // Keys: auth_method::<name> → JSON (keyed by name, no secondary index)
    // ========================================================================

    pub fn get_auth_method(&self, name: &str) -> Option<AuthMethod> {
        match self {
            Self::Memory { auth_methods, .. } => auth_methods.get(name).map(|e| e.value().clone()),
            Self::Persistent { .. } => self.rocks_get(&format!("auth_method::{}", name)),
        }
    }

    pub fn put_auth_method(&self, method: &AuthMethod) {
        match self {
            Self::Memory { auth_methods, .. } => {
                auth_methods.insert(method.name.clone(), method.clone());
            }
            Self::Persistent { .. } => {
                self.rocks_put(&format!("auth_method::{}", method.name), method);
            }
        }
    }

    pub fn remove_auth_method(&self, name: &str) -> bool {
        match self {
            Self::Memory { auth_methods, .. } => auth_methods.remove(name).is_some(),
            Self::Persistent { .. } => {
                let key = format!("auth_method::{}", name);
                let existed = self.rocks_get_raw(&key).is_some();
                if existed {
                    self.rocks_delete(&key);
                }
                existed
            }
        }
    }

    pub fn list_auth_methods(&self) -> Vec<AuthMethod> {
        match self {
            Self::Memory { auth_methods, .. } => {
                auth_methods.iter().map(|e| e.value().clone()).collect()
            }
            Self::Persistent { .. } => self.rocks_list("auth_method::"),
        }
    }

    // ========================================================================
    // Binding rule operations
    //
    // Keys: binding_rule::<id> → JSON
    // ========================================================================

    pub fn get_binding_rule(&self, id: &str) -> Option<BindingRule> {
        match self {
            Self::Memory { binding_rules, .. } => binding_rules.get(id).map(|e| e.value().clone()),
            Self::Persistent { .. } => self.rocks_get(&format!("binding_rule::{}", id)),
        }
    }

    pub fn put_binding_rule(&self, rule: &BindingRule) {
        match self {
            Self::Memory { binding_rules, .. } => {
                binding_rules.insert(rule.id.clone(), rule.clone());
            }
            Self::Persistent { .. } => {
                self.rocks_put(&format!("binding_rule::{}", rule.id), rule);
            }
        }
    }

    pub fn remove_binding_rule(&self, id: &str) -> bool {
        match self {
            Self::Memory { binding_rules, .. } => binding_rules.remove(id).is_some(),
            Self::Persistent { .. } => {
                let key = format!("binding_rule::{}", id);
                let existed = self.rocks_get_raw(&key).is_some();
                if existed {
                    self.rocks_delete(&key);
                }
                existed
            }
        }
    }

    pub fn list_binding_rules(&self) -> Vec<BindingRule> {
        match self {
            Self::Memory { binding_rules, .. } => {
                binding_rules.iter().map(|e| e.value().clone()).collect()
            }
            Self::Persistent { .. } => self.rocks_list("binding_rule::"),
        }
    }
}
