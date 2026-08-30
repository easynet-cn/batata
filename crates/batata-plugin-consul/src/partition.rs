//! Consul Partition service (simplified Enterprise feature)
//!
//! Provides partition CRUD operations compatible with the Consul Enterprise API.
//! The "default" partition always exists and cannot be deleted.
//! Delete is soft-delete (sets `DeletedAt` timestamp) rather than hard removal.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;
use crate::model::ConsulErrorBody;
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};
use tracing::error;

/// Default partition name (always exists, cannot be deleted or updated)
pub const DEFAULT_PARTITION: &str = "default";

/// HTTP status code paired with an error message for service-level errors.
pub type PartitionError = (u16, String);

/// Consul Partition (simplified Enterprise feature)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[derive(Default)]
pub struct Partition {
/// The `name` field.
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
/// The `description` field.
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
/// The `deleted_at` field.
    pub deleted_at: Option<String>,
    #[serde(default)]
/// The `disable_gossip` field.
    pub disable_gossip: bool,
    #[serde(default)]
/// The `create_index` field.
    pub create_index: u64,
    #[serde(default)]
/// The `modify_index` field.
    pub modify_index: u64,
}

/// Consul Partition service
#[derive(Clone)]
pub struct ConsulPartitionService {
    partitions: Arc<DashMap<String, Partition>>,
    index_provider: ConsulIndexProvider,
    /// Optional Raft writer for cluster-mode replication
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl ConsulPartitionService {
/// The `new` associated function.
    pub fn new(index_provider: ConsulIndexProvider) -> Self {
        let partitions = Arc::new(DashMap::new());
        // "default" partition always exists
        partitions.insert(
            DEFAULT_PARTITION.to_string(),
            Partition {
                name: DEFAULT_PARTITION.to_string(),
                description: "Builtin Default Partition".to_string(),
                deleted_at: None,
                disable_gossip: false,
                create_index: 1,
                modify_index: 1,
            },
        );
        Self {
            partitions,
            index_provider,
            raft_node: None,
        }
    }

    /// Create a partition service with Raft-replicated storage (cluster mode).
    pub fn with_raft(
        raft_node: Arc<ConsulRaftWriter>,
        index_provider: ConsulIndexProvider,
    ) -> Self {
        let mut svc = Self::new(index_provider);
        svc.raft_node = Some(raft_node);
        svc
    }

    /// Check if a partition exists (and is not deleted)
    pub fn exists(&self, name: &str) -> bool {
        self.partitions
            .get(name)
            .map(|r| r.deleted_at.is_none())
            .unwrap_or(false)
    }

    /// Get a partition by name (including deleted ones)
    pub fn get(&self, name: &str) -> Option<Partition> {
        self.partitions.get(name).map(|r| r.clone())
    }

    /// List all partitions (excluding soft-deleted ones)
    pub fn list(&self) -> Vec<Partition> {
        let mut result: Vec<Partition> = self
            .partitions
            .iter()
            .filter(|r| r.deleted_at.is_none())
            .map(|r| r.clone())
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }

    /// Validate a partition name: lowercase alphanumeric and hyphens only
    fn validate_name(name: &str) -> Result<(), PartitionError> {
        if name.is_empty() {
            return Err((400, "Must specify a Name for Partition".to_string()));
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err((
                400,
                "Partition name may only contain lowercase alphanumeric characters and hyphens"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Internal upsert: writes to in-memory, sends via Raft, increments index
    async fn upsert(&self, partition: Partition) -> Partition {
        let index = self.index_provider.current_index(ConsulTable::Catalog);
        let existing = self.partitions.get(&partition.name);
        let create_index = existing.as_ref().map(|e| e.create_index).unwrap_or(index);
        drop(existing);

        let stored = Partition {
            create_index,
            modify_index: index,
            ..partition
        };
        self.partitions.insert(stored.name.clone(), stored.clone());
        if let Some(ref raft) = self.raft_node {
            let partition_json = serde_json::to_string(&stored).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::PartitionUpsert {
                    name: stored.name.clone(),
                    partition_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft PartitionUpsert rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft PartitionUpsert failed: {}", e);
                }
                _ => {}
            }
        }
        self.index_provider.increment(ConsulTable::Catalog);
        stored
    }

    /// Create a new partition.
    /// Returns the created partition, or an error with HTTP status code.
    pub async fn create_partition(
        &self,
        req: Partition,
    ) -> Result<Partition, PartitionError> {
        Self::validate_name(&req.name)?;

        // Cannot create "default" — it already exists
        if req.name == DEFAULT_PARTITION {
            return Err((
                409,
                format!("Partition '{}' already exists", DEFAULT_PARTITION),
            ));
        }

        // Check for duplicate (non-deleted)
        if self.exists(&req.name) {
            return Err((409, format!("Partition '{}' already exists", req.name)));
        }

        // If a soft-deleted partition with the same name exists, remove it
        // so the new one gets a fresh create_index
        if let Some(existing) = self.partitions.get(&req.name) {
            if existing.deleted_at.is_some() {
                drop(existing);
                self.partitions.remove(&req.name);
            }
        }

        let created = self.upsert(req).await;
        Ok(created)
    }

    /// Update an existing partition.
    /// Returns the updated partition, or an error with HTTP status code.
    pub async fn update_partition(
        &self,
        name: &str,
        req: Partition,
    ) -> Result<Partition, PartitionError> {
        // "default" partition cannot be updated
        if name == DEFAULT_PARTITION {
            return Err((
                400,
                "The default partition cannot be modified".to_string(),
            ));
        }

        // Check existence (including soft-deleted)
        if !self.partitions.contains_key(name) {
            return Err((404, format!("Partition '{}' not found", name)));
        }

        let mut updated = req;
        updated.name = name.to_string();
        // Preserve deleted_at — update should not un-delete
        if let Some(existing) = self.partitions.get(name) {
            if existing.deleted_at.is_some() {
                updated.deleted_at = existing.deleted_at.clone();
            }
        }

        let result = self.upsert(updated).await;
        Ok(result)
    }

    /// Read a partition by name.
    /// Returns the partition (including soft-deleted ones with DeletedAt set).
    pub fn read_partition(&self, name: &str) -> Option<Partition> {
        self.get(name)
    }

    /// Delete a partition (soft delete — sets DeletedAt timestamp).
    /// Returns Ok(()) on success, or an error with HTTP status code.
    pub async fn delete_partition(&self, name: &str) -> Result<(), PartitionError> {
        // "default" partition cannot be deleted
        if name == DEFAULT_PARTITION {
            return Err((
                400,
                "Cannot delete the default partition".to_string(),
            ));
        }

        // Check existence (including soft-deleted)
        let existing = match self.partitions.get(name) {
            Some(r) => r.clone(),
            None => {
                return Err((404, format!("Partition '{}' not found", name)));
            }
        };

        // Already soft-deleted — return success (idempotent)
        if existing.deleted_at.is_some() {
            return Ok(());
        }

        // Soft delete: set deleted_at timestamp
        let now = chrono::Utc::now().to_rfc3339();
        let deleted = Partition {
            deleted_at: Some(now),
            ..existing
        };
        self.partitions.insert(name.to_string(), deleted.clone());

        if let Some(ref raft) = self.raft_node {
            let partition_json = serde_json::to_string(&deleted).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::PartitionUpsert {
                    name: name.to_string(),
                    partition_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft PartitionDelete (upsert) rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft PartitionDelete (upsert) failed: {}", e);
                }
                _ => {}
            }
        }
        self.index_provider.increment(ConsulTable::Catalog);
        Ok(())
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// GET /v1/partitions - List all partitions
pub async fn list_partitions(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Partitions));
    consul_ok(&meta).json(partition_service.list())
}

/// GET /v1/partition/{name} - Read a partition
pub async fn read_partition(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let name = path.into_inner();
    match partition_service.read_partition(&name) {
        Some(partition) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Partitions));
            consul_ok(&meta).json(partition)
        }
        None => HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Partition '{}' not found", name))),
    }
}

/// PUT /v1/partition - Create a partition
pub async fn create_partition(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    body: web::Json<Partition>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let partition = body.into_inner();
    match partition_service.create_partition(partition).await {
        Ok(created) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Partitions));
            consul_ok(&meta).json(created)
        }
        Err((status, msg)) => {
            if status == 404 {
                HttpResponse::NotFound().consul_error(ConsulError::new(msg))
            } else if status == 409 {
                HttpResponse::Conflict().consul_error(ConsulError::new(msg))
            } else {
                HttpResponse::BadRequest().consul_error(ConsulError::new(msg))
            }
        }
    }
}

/// PUT /v1/partition/{name} - Update a partition
pub async fn update_partition(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<Partition>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let name = path.into_inner();
    let partition = body.into_inner();
    match partition_service.update_partition(&name, partition).await {
        Ok(updated) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Partitions));
            consul_ok(&meta).json(updated)
        }
        Err((status, msg)) => {
            if status == 404 {
                HttpResponse::NotFound().consul_error(ConsulError::new(msg))
            } else if status == 409 {
                HttpResponse::Conflict().consul_error(ConsulError::new(msg))
            } else {
                HttpResponse::BadRequest().consul_error(ConsulError::new(msg))
            }
        }
    }
}

/// DELETE /v1/partition/{name} - Delete a partition (soft delete)
pub async fn delete_partition(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let name = path.into_inner();
    match partition_service.delete_partition(&name).await {
        Ok(()) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Partitions));
            consul_ok(&meta).finish()
        }
        Err((status, msg)) => {
            if status == 404 {
                HttpResponse::NotFound().consul_error(ConsulError::new(msg))
            } else {
                HttpResponse::BadRequest().consul_error(ConsulError::new(msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> ConsulPartitionService {
        ConsulPartitionService::new(ConsulIndexProvider::new())
    }

    #[test]
    fn test_default_partition_exists() {
        let svc = create_test_service();
        assert!(svc.exists("default"));
        let p = svc.get("default").unwrap();
        assert_eq!(p.name, "default");
        assert_eq!(p.description, "Builtin Default Partition");
    }

    #[tokio::test]
    async fn test_create_partition() {
        let svc = create_test_service();
        let p = Partition {
            name: "test-p".to_string(),
            description: "Test partition".to_string(),
            ..Default::default()
        };
        let created = svc.create_partition(p).await.unwrap();
        assert_eq!(created.name, "test-p");
        assert!(created.create_index > 0);
        assert!(svc.exists("test-p"));
    }

    #[tokio::test]
    async fn test_create_default_fails() {
        let svc = create_test_service();
        let p = Partition {
            name: "default".to_string(),
            ..Default::default()
        };
        let result = svc.create_partition(p).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, 409);
    }

    #[tokio::test]
    async fn test_create_duplicate_fails() {
        let svc = create_test_service();
        let p = Partition {
            name: "dup".to_string(),
            ..Default::default()
        };
        svc.create_partition(p.clone()).await.unwrap();
        let result = svc.create_partition(p).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, 409);
    }

    #[tokio::test]
    async fn test_read_partition() {
        let svc = create_test_service();
        svc.create_partition(Partition {
            name: "read-me".to_string(),
            description: "Readable".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let p = svc.read_partition("read-me").unwrap();
        assert_eq!(p.name, "read-me");
        assert_eq!(p.description, "Readable");
    }

    #[tokio::test]
    async fn test_read_not_found() {
        let svc = create_test_service();
        assert!(svc.read_partition("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_update_partition() {
        let svc = create_test_service();
        svc.create_partition(Partition {
            name: "update-me".to_string(),
            description: "Original".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let updated = svc
            .update_partition(
                "update-me",
                Partition {
                    name: "update-me".to_string(),
                    description: "Updated".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.description, "Updated");
        assert!(updated.modify_index >= updated.create_index);
    }

    #[tokio::test]
    async fn test_update_default_fails() {
        let svc = create_test_service();
        let result = svc
            .update_partition(
                "default",
                Partition {
                    name: "default".to_string(),
                    description: "Hacked".to_string(),
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_partition() {
        let svc = create_test_service();
        svc.create_partition(Partition {
            name: "deleteme".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        assert!(svc.exists("deleteme"));

        svc.delete_partition("deleteme").await.unwrap();
        // Soft delete: partition still in storage but marked deleted
        assert!(!svc.exists("deleteme"));
        let p = svc.get("deleteme").unwrap();
        assert!(p.deleted_at.is_some());
    }

    #[tokio::test]
    async fn test_delete_default_fails() {
        let svc = create_test_service();
        let result = svc.delete_partition("default").await;
        assert!(result.is_err());
        assert!(svc.exists("default"));
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let svc = create_test_service();
        let result = svc.delete_partition("nonexistent").await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn test_list_partitions() {
        let svc = create_test_service();
        svc.create_partition(Partition {
            name: "alpha".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        svc.create_partition(Partition {
            name: "beta".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let list = svc.list();
        assert_eq!(list.len(), 3); // default + alpha + beta
        assert_eq!(list[0].name, "alpha");
        assert_eq!(list[1].name, "beta");
        assert_eq!(list[2].name, "default");
    }

    #[tokio::test]
    async fn test_list_excludes_deleted() {
        let svc = create_test_service();
        svc.create_partition(Partition {
            name: "temp".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        svc.delete_partition("temp").await.unwrap();

        let list = svc.list();
        // default should still be there, "temp" should be excluded
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "default");
    }

    #[tokio::test]
    async fn test_recreate_after_delete() {
        let svc = create_test_service();
        svc.create_partition(Partition {
            name: "recycle".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        svc.delete_partition("recycle").await.unwrap();
        assert!(!svc.exists("recycle"));

        // Should be able to create again
        let result = svc
            .create_partition(Partition {
                name: "recycle".to_string(),
                description: "Reborn".to_string(),
                ..Default::default()
            })
            .await;
        assert!(result.is_ok());
        let p = result.unwrap();
        assert_eq!(p.description, "Reborn");
    }
}
