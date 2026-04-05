//! Consul Namespace service (Enterprise feature)
//!
//! Provides namespace CRUD operations compatible with the Consul Enterprise API.
//! The "default" namespace always exists and cannot be deleted.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::acl::{AclService, ResourceType};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;
use crate::raft::ConsulRaftWriter;

/// Default namespace name (always exists, cannot be deleted)
pub const DEFAULT_NAMESPACE: &str = "default";

/// Consul Namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Namespace {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(rename = "ACLs", default, skip_serializing_if = "Option::is_none")]
    pub acls: Option<NamespaceACLConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<std::collections::HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partition: String,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}

/// Namespace ACL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NamespaceACLConfig {
    #[serde(default)]
    pub policy_defaults: Vec<ACLLink>,
    #[serde(default)]
    pub role_defaults: Vec<ACLLink>,
}

/// ACL link reference
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ACLLink {
    #[serde(rename = "ID", default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

/// Consul Namespace service
#[derive(Clone)]
pub struct ConsulNamespaceService {
    namespaces: Arc<DashMap<String, Namespace>>,
    index_provider: ConsulIndexProvider,
    /// Optional Raft writer for cluster-mode replication
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl ConsulNamespaceService {
    pub fn new(index_provider: ConsulIndexProvider) -> Self {
        let namespaces = Arc::new(DashMap::new());
        // "default" namespace always exists
        namespaces.insert(
            DEFAULT_NAMESPACE.to_string(),
            Namespace {
                name: DEFAULT_NAMESPACE.to_string(),
                description: "Builtin Default Namespace".to_string(),
                acls: None,
                meta: Some({
                    let mut m = std::collections::HashMap::new();
                    m.insert("external-source".to_string(), "batata".to_string());
                    m
                }),
                deleted_at: None,
                partition: "default".to_string(),
                create_index: 1,
                modify_index: 1,
            },
        );
        Self {
            namespaces,
            index_provider,
            raft_node: None,
        }
    }

    /// Create a namespace service with Raft-replicated storage (cluster mode).
    pub fn with_raft(
        raft_node: Arc<ConsulRaftWriter>,
        index_provider: ConsulIndexProvider,
    ) -> Self {
        let mut svc = Self::new(index_provider);
        svc.raft_node = Some(raft_node);
        svc
    }

    /// Check if a namespace exists
    pub fn exists(&self, name: &str) -> bool {
        self.namespaces.contains_key(name)
    }

    /// Resolve a namespace name, defaulting to "default" if empty/None
    pub fn resolve(&self, ns: Option<&str>) -> String {
        match ns {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => DEFAULT_NAMESPACE.to_string(),
        }
    }

    /// Get a namespace by name
    pub fn get(&self, name: &str) -> Option<Namespace> {
        self.namespaces.get(name).map(|r| r.clone())
    }

    /// List all namespaces
    pub fn list(&self) -> Vec<Namespace> {
        let mut result: Vec<Namespace> = self.namespaces.iter().map(|r| r.clone()).collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }

    /// Create or update a namespace
    pub fn upsert(&self, ns: Namespace) -> Namespace {
        let index = self.index_provider.current_index(ConsulTable::Catalog);
        let existing = self.namespaces.get(&ns.name);
        let create_index = existing.as_ref().map(|e| e.create_index).unwrap_or(index);
        drop(existing);

        let stored = Namespace {
            create_index,
            modify_index: index,
            ..ns
        };
        self.namespaces.insert(stored.name.clone(), stored.clone());
        self.index_provider.increment(ConsulTable::Catalog);
        stored
    }

    /// Delete a namespace. Returns true if deleted, false if not found.
    /// The "default" namespace cannot be deleted.
    pub fn delete(&self, name: &str) -> Result<(), String> {
        if name == DEFAULT_NAMESPACE {
            return Err("Cannot delete the default namespace".to_string());
        }
        if self.namespaces.remove(name).is_some() {
            self.index_provider.increment(ConsulTable::Catalog);
            Ok(())
        } else {
            Err(format!("Namespace '{}' not found", name))
        }
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// GET /v1/namespaces - List all namespaces
pub async fn list_namespaces(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(ns_service.list())
}

/// GET /v1/namespace/{name} - Read a namespace
pub async fn read_namespace(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let name = path.into_inner();
    match ns_service.get(&name) {
        Some(ns) => HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .json(ns),
        None => HttpResponse::NotFound()
            .json(ConsulError::new(format!("Namespace '{}' not found", name))),
    }
}

/// PUT /v1/namespace - Create a namespace
pub async fn create_namespace(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    body: web::Json<Namespace>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let ns = body.into_inner();
    if ns.name.is_empty() {
        return HttpResponse::BadRequest().json(ConsulError::new(
            "Must specify a Name for Namespace creation",
        ));
    }

    let created = ns_service.upsert(ns);
    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(created)
}

/// PUT /v1/namespace/{name} - Update a namespace
pub async fn update_namespace(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<Namespace>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let name = path.into_inner();
    let mut ns = body.into_inner();
    ns.name = name;

    let updated = ns_service.upsert(ns);
    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(updated)
}

/// DELETE /v1/namespace/{name} - Delete a namespace
pub async fn delete_namespace(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let name = path.into_inner();
    match ns_service.delete(&name) {
        Ok(()) => HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .finish(),
        Err(msg) => HttpResponse::BadRequest().json(ConsulError::new(msg)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> ConsulNamespaceService {
        ConsulNamespaceService::new(ConsulIndexProvider::new())
    }

    #[test]
    fn test_default_namespace_exists() {
        let svc = create_test_service();
        assert!(svc.exists("default"));
        let ns = svc.get("default").unwrap();
        assert_eq!(ns.name, "default");
        assert_eq!(ns.description, "Builtin Default Namespace");
    }

    #[test]
    fn test_create_namespace() {
        let svc = create_test_service();
        let ns = Namespace {
            name: "test-ns".to_string(),
            description: "Test namespace".to_string(),
            ..Default::default()
        };
        let created = svc.upsert(ns);
        assert_eq!(created.name, "test-ns");
        assert!(created.create_index > 0);
        assert!(svc.exists("test-ns"));
    }

    #[test]
    fn test_list_namespaces() {
        let svc = create_test_service();
        svc.upsert(Namespace {
            name: "alpha".to_string(),
            ..Default::default()
        });
        svc.upsert(Namespace {
            name: "beta".to_string(),
            ..Default::default()
        });

        let list = svc.list();
        assert_eq!(list.len(), 3); // default + alpha + beta
        assert_eq!(list[0].name, "alpha");
        assert_eq!(list[1].name, "beta");
        assert_eq!(list[2].name, "default");
    }

    #[test]
    fn test_delete_namespace() {
        let svc = create_test_service();
        svc.upsert(Namespace {
            name: "deleteme".to_string(),
            ..Default::default()
        });
        assert!(svc.exists("deleteme"));

        assert!(svc.delete("deleteme").is_ok());
        assert!(!svc.exists("deleteme"));
    }

    #[test]
    fn test_cannot_delete_default() {
        let svc = create_test_service();
        assert!(svc.delete("default").is_err());
        assert!(svc.exists("default"));
    }

    #[test]
    fn test_update_namespace() {
        let svc = create_test_service();
        svc.upsert(Namespace {
            name: "update-me".to_string(),
            description: "Original".to_string(),
            ..Default::default()
        });

        let updated = svc.upsert(Namespace {
            name: "update-me".to_string(),
            description: "Updated".to_string(),
            ..Default::default()
        });
        assert_eq!(updated.description, "Updated");
        // create_index should be preserved
        assert!(updated.modify_index >= updated.create_index);
    }

    #[test]
    fn test_resolve_namespace() {
        let svc = create_test_service();
        assert_eq!(svc.resolve(None), "default");
        assert_eq!(svc.resolve(Some("")), "default");
        assert_eq!(svc.resolve(Some("custom")), "custom");
    }

    #[test]
    fn test_delete_nonexistent() {
        let svc = create_test_service();
        assert!(svc.delete("nonexistent").is_err());
    }
}

impl Default for Namespace {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            acls: None,
            meta: None,
            deleted_at: None,
            partition: String::new(),
            create_index: 0,
            modify_index: 0,
        }
    }
}
