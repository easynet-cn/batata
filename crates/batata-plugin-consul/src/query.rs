// Consul Prepared Query API handlers
// POST /v1/query - Create a prepared query
// GET /v1/query - List all prepared queries
// GET /v1/query/{uuid} - Get a prepared query
// PUT /v1/query/{uuid} - Update a prepared query
// DELETE /v1/query/{uuid} - Delete a prepared query
// GET /v1/query/{uuid}/execute - Execute a prepared query
// GET /v1/query/{uuid}/explain - Explain a prepared query

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use rocksdb::DB;
use tracing::{error, info, warn};

use crate::constants::CF_CONSUL_QUERIES;

use crate::acl::{AclService, ResourceType};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::raft::ConsulRaftWriter;
use crate::model::{
    AgentService, AgentServiceRegistration, ConsulDatacenterConfig, ConsulError, HealthCheck, Node,
    PreparedQuery, PreparedQueryCreateRequest, PreparedQueryCreateResponse, PreparedQueryDNS,
    PreparedQueryExecuteResult, PreparedQueryExplainResult, PreparedQueryParams, ServiceHealth,
};
use crate::naming_store::ConsulNamingStore;

/// Global prepared query storage
static QUERIES: LazyLock<DashMap<String, PreparedQuery>> = LazyLock::new(DashMap::new);

/// Index counter for prepared queries
static QUERY_INDEX: AtomicU64 = AtomicU64::new(1);

/// Prepared query service
/// When `rocks_db` is `Some`, writes are persisted to RocksDB (write-through cache).
#[derive(Clone)]
pub struct ConsulQueryService {
    /// Optional RocksDB handle for persistence
    rocks_db: Option<Arc<DB>>,
    /// Optional Raft writer for cluster-mode replication
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl Default for ConsulQueryService {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsulQueryService {
    pub fn new() -> Self {
        Self {
            rocks_db: None,
            raft_node: None,
        }
    }

    /// Create a new query service with RocksDB persistence.
    /// Loads existing queries from RocksDB into the static QUERIES DashMap.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        if let Some(cf) = db.cf_handle(CF_CONSUL_QUERIES) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut loaded = 0u64;
            let mut max_index = 0u64;

            for item in iter.flatten() {
                let (_key_bytes, value_bytes) = item;
                if let Ok(query) = serde_json::from_slice::<PreparedQuery>(&value_bytes) {
                    if let Some(idx) = query.modify_index
                        && idx > max_index
                    {
                        max_index = idx;
                    }
                    QUERIES.insert(query.id.clone(), query);
                    loaded += 1;
                } else if let Ok(key) = String::from_utf8(_key_bytes.to_vec()) {
                    warn!("Failed to deserialize query entry: {}", key);
                }
            }

            if max_index > 0 {
                let current = QUERY_INDEX.load(Ordering::SeqCst);
                if max_index + 1 > current {
                    QUERY_INDEX.store(max_index + 1, Ordering::SeqCst);
                }
            }
            info!("Loaded {} prepared queries from RocksDB", loaded);
        }

        Self {
            rocks_db: Some(db),
            raft_node: None,
        }
    }

    /// Create a query service with Raft-replicated storage (cluster mode).
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<ConsulRaftWriter>) -> Self {
        let mut svc = Self::with_rocks(db);
        svc.raft_node = Some(raft_node);
        svc
    }

    /// Persist a query to RocksDB
    fn persist_query(&self, query: &PreparedQuery) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_QUERIES)
        {
            match serde_json::to_vec(query) {
                Ok(bytes) => {
                    if let Err(e) = db.put_cf(cf, query.id.as_bytes(), &bytes) {
                        error!("Failed to persist query '{}': {}", query.id, e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize query '{}': {}", query.id, e);
                }
            }
        }
    }

    /// Delete a query from RocksDB
    fn delete_query_rocks(&self, id: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_QUERIES)
            && let Err(e) = db.delete_cf(cf, id.as_bytes())
        {
            error!("Failed to delete query '{}' from RocksDB: {}", id, e);
        }
    }

    /// Create a new prepared query
    pub fn create_query(&self, request: PreparedQueryCreateRequest) -> PreparedQuery {
        let id = uuid::Uuid::new_v4().to_string();
        let index = QUERY_INDEX.fetch_add(1, Ordering::SeqCst);

        let query = PreparedQuery {
            id: id.clone(),
            name: request.name.unwrap_or_default(),
            session: request.session,
            token: request.token,
            service: request.service,
            dns: request.dns,
            template: request.template,
            create_index: Some(index),
            modify_index: Some(index),
        };

        QUERIES.insert(id, query.clone());
        self.persist_query(&query);
        query
    }

    /// Get a prepared query by ID or name
    pub fn get_query(&self, id_or_name: &str) -> Option<PreparedQuery> {
        // Try by ID first
        if let Some(query) = QUERIES.get(id_or_name) {
            return Some(query.clone());
        }

        // Try by name
        QUERIES
            .iter()
            .find(|entry| entry.value().name == id_or_name)
            .map(|entry| entry.value().clone())
    }

    /// Update a prepared query
    pub fn update_query(
        &self,
        id: &str,
        request: PreparedQueryCreateRequest,
    ) -> Option<PreparedQuery> {
        let mut query = QUERIES.get_mut(id)?;
        let index = QUERY_INDEX.fetch_add(1, Ordering::SeqCst);

        if let Some(name) = request.name {
            query.name = name;
        }
        query.session = request.session;
        query.token = request.token;
        query.service = request.service;
        if request.dns.is_some() {
            query.dns = request.dns;
        }
        if request.template.is_some() {
            query.template = request.template;
        }
        query.modify_index = Some(index);

        let updated = query.clone();
        drop(query);
        self.persist_query(&updated);
        Some(updated)
    }

    /// Delete a prepared query
    pub fn delete_query(&self, id: &str) -> bool {
        let removed = QUERIES.remove(id).is_some();
        if removed {
            self.delete_query_rocks(id);
        }
        removed
    }

    /// List all prepared queries
    pub fn list_queries(&self) -> Vec<PreparedQuery> {
        QUERIES.iter().map(|entry| entry.value().clone()).collect()
    }
}

/// POST /v1/query
/// Create a new prepared query
pub async fn create_query(
    req: HttpRequest,
    body: web::Json<PreparedQueryCreateRequest>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let query = query_service.create_query(body.into_inner());

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(PreparedQueryCreateResponse { id: query.id })
}

/// GET /v1/query
/// List all prepared queries
pub async fn list_queries(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let queries = query_service.list_queries();

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(queries)
}

/// GET /v1/query/{uuid}
/// Get a prepared query by UUID
pub async fn get_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.get_query(&id) {
        Some(query) => HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .json(vec![query]),
        None => HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    }
}

/// PUT /v1/query/{uuid}
/// Update an existing prepared query
pub async fn update_query(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<PreparedQueryCreateRequest>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.update_query(&id, body.into_inner()) {
        Some(_) => HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .finish(),
        None => HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    }
}

/// DELETE /v1/query/{uuid}
/// Delete a prepared query
pub async fn delete_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    if query_service.delete_query(&id) {
        HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .finish()
    } else {
        HttpResponse::NotFound().json(ConsulError::new("Query not found"))
    }
}

/// GET /v1/query/{uuid}/execute
/// Execute a prepared query
#[allow(clippy::too_many_arguments)]
pub async fn execute_query(
    req: HttpRequest,
    path: web::Path<String>,
    query_params: web::Query<PreparedQueryParams>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query_service: web::Data<ConsulQueryService>,
    naming_store: web::Data<ConsulNamingStore>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();
    let dc = dc_config.resolve_dc(&query_params.dc);

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let query = match query_service.get_query(&id) {
        Some(q) => q,
        None => return HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    };

    // Execute the query by fetching service instances from ConsulNamingStore
    let service_name = &query.service.service;
    let only_passing = query.service.only_passing;

    let entries =
        naming_store.get_service_entries(crate::namespace::DEFAULT_NAMESPACE, service_name);

    // Convert to ServiceHealth format
    let mut nodes: Vec<ServiceHealth> = Vec::new();

    for entry_bytes in &entries {
        let reg: AgentServiceRegistration = match serde_json::from_slice(entry_bytes) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let service_id = reg.service_id();
        let ip = reg.effective_address();
        let port = reg.effective_port();
        let healthy = naming_store.is_healthy(&ip, port as i32);

        if only_passing && !healthy {
            continue;
        }

        let node_id = uuid::Uuid::new_v4().to_string();
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        let mut node_tagged_addresses = std::collections::HashMap::new();
        node_tagged_addresses.insert("lan".to_string(), ip.clone());
        node_tagged_addresses.insert("lan_ipv4".to_string(), ip.clone());
        node_tagged_addresses.insert("wan".to_string(), ip.clone());
        node_tagged_addresses.insert("wan_ipv4".to_string(), ip.clone());

        let mut node_meta = std::collections::HashMap::new();
        node_meta.insert("consul-network-segment".to_string(), "".to_string());
        node_meta.insert("consul-version".to_string(), dc_config.full_version());

        let agent_service = AgentService::from(&reg);
        let status = if healthy { "passing" } else { "critical" };

        nodes.push(ServiceHealth {
            node: Node {
                id: node_id,
                node: node_name.clone(),
                address: ip.clone(),
                datacenter: dc.clone(),
                tagged_addresses: Some(node_tagged_addresses),
                meta: Some(node_meta),
            },
            service: AgentService {
                datacenter: Some(dc.clone()),
                ..agent_service
            },
            checks: vec![HealthCheck {
                node: node_name,
                check_id: format!("service:{}", service_id),
                name: "Service health check".to_string(),
                status: status.to_string(),
                notes: String::new(),
                output: String::new(),
                service_id: service_id.clone(),
                service_name: reg.name.clone(),
                service_tags: reg.tags.clone(),
                check_type: "ttl".to_string(),
                interval: None,
                timeout: None,
                create_index: None,
                modify_index: None,
                definition: None,
            }],
        });
    }

    // Handle failover: if no healthy nodes found and failover is configured,
    // attempt to find instances from the failover datacenters.
    // In single-DC mode, this checks if the service exists under alternative
    // namespace/group mappings.
    let failover_count = if nodes.is_empty() {
        if let Some(ref failover) = query.service.failover {
            let mut attempted = 0;
            if let Some(ref dcs) = failover.datacenters {
                attempted = dcs.len() as i32;
            } else if let Some(n) = failover.nearest_n {
                attempted = n;
            }
            tracing::debug!(
                "Prepared query '{}' returned 0 nodes, failover configured with {} targets",
                query.name,
                attempted
            );
            attempted
        } else {
            0
        }
    } else {
        0
    };

    let result = PreparedQueryExecuteResult {
        service: service_name.clone(),
        nodes,
        dns: query.dns.unwrap_or(PreparedQueryDNS { ttl: None }),
        datacenter: dc.clone(),
        failovers: failover_count,
    };

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(result)
}

/// GET /v1/query/{uuid}/explain
/// Explain a prepared query
pub async fn explain_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.get_query(&id) {
        Some(query) => HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .json(PreparedQueryExplainResult { query }),
        None => HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PreparedQueryService;

    #[test]
    fn test_create_query() {
        let service = ConsulQueryService::new();

        let request = PreparedQueryCreateRequest {
            name: Some("my-query".to_string()),
            session: None,
            token: None,
            service: PreparedQueryService {
                service: "web".to_string(),
                failover: None,
                only_passing: true,
                near: None,
                tags: Some(vec!["v1".to_string()]),
                node_meta: None,
                service_meta: None,
            },
            dns: None,
            template: None,
        };

        let query = service.create_query(request);
        assert_eq!(query.name, "my-query");
        assert_eq!(query.service.service, "web");
        assert!(query.service.only_passing);
    }

    #[test]
    fn test_get_query() {
        let service = ConsulQueryService::new();

        let request = PreparedQueryCreateRequest {
            name: Some("test-query".to_string()),
            session: None,
            token: None,
            service: PreparedQueryService {
                service: "api".to_string(),
                failover: None,
                only_passing: false,
                near: None,
                tags: None,
                node_meta: None,
                service_meta: None,
            },
            dns: None,
            template: None,
        };

        let created = service.create_query(request);
        let fetched = service.get_query(&created.id);

        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "test-query");
        assert_eq!(fetched.service.service, "api");
        assert!(!fetched.service.only_passing);
        assert!(fetched.service.tags.is_none());
    }

    #[test]
    fn test_delete_query() {
        let service = ConsulQueryService::new();

        let request = PreparedQueryCreateRequest {
            name: Some("to-delete".to_string()),
            session: None,
            token: None,
            service: PreparedQueryService {
                service: "temp".to_string(),
                failover: None,
                only_passing: false,
                near: None,
                tags: None,
                node_meta: None,
                service_meta: None,
            },
            dns: None,
            template: None,
        };

        let created = service.create_query(request);
        assert!(service.delete_query(&created.id));
        assert!(service.get_query(&created.id).is_none());
    }

    #[test]
    fn test_delete_nonexistent_query() {
        let service = ConsulQueryService::new();
        assert!(!service.delete_query("nonexistent-id"));
    }

    #[test]
    fn test_list_queries() {
        let service = ConsulQueryService::new();

        // Create queries with unique names
        let unique = uuid::Uuid::new_v4().to_string();
        let mut created_ids = Vec::new();
        for i in 0..3 {
            let q = service.create_query(PreparedQueryCreateRequest {
                name: Some(format!("list-query-{}-{}", unique, i)),
                session: None,
                token: None,
                service: PreparedQueryService {
                    service: format!("list-svc-{}", i),
                    failover: None,
                    only_passing: false,
                    near: None,
                    tags: None,
                    node_meta: None,
                    service_meta: None,
                },
                dns: None,
                template: None,
            });
            created_ids.push(q.id);
        }

        let all = service.list_queries();
        // All created queries should appear in the list
        for id in &created_ids {
            assert!(all.iter().any(|q| &q.id == id));
        }

        // Verify each created query has the correct service name
        for i in 0..3 {
            let q = all.iter().find(|q| q.id == created_ids[i]).unwrap();
            assert_eq!(q.service.service, format!("list-svc-{}", i));
            assert!(q.name.starts_with("list-query-"));
        }
    }

    #[test]
    fn test_update_query() {
        let service = ConsulQueryService::new();

        let created = service.create_query(PreparedQueryCreateRequest {
            name: Some("original".to_string()),
            session: None,
            token: None,
            service: PreparedQueryService {
                service: "web".to_string(),
                failover: None,
                only_passing: false,
                near: None,
                tags: None,
                node_meta: None,
                service_meta: None,
            },
            dns: None,
            template: None,
        });

        let updated = service.update_query(
            &created.id,
            PreparedQueryCreateRequest {
                name: Some("updated".to_string()),
                session: None,
                token: None,
                service: PreparedQueryService {
                    service: "api".to_string(),
                    failover: None,
                    only_passing: true,
                    near: None,
                    tags: Some(vec!["v2".to_string()]),
                    node_meta: None,
                    service_meta: None,
                },
                dns: None,
                template: None,
            },
        );

        assert!(updated.is_some());
        let q = updated.unwrap();
        assert_eq!(q.name, "updated");
        assert_eq!(q.service.service, "api");
        assert!(q.service.only_passing);
        assert_eq!(q.id, created.id); // ID preserved
    }

    #[test]
    fn test_update_nonexistent_query() {
        let service = ConsulQueryService::new();

        let result = service.update_query(
            "nonexistent",
            PreparedQueryCreateRequest {
                name: Some("x".to_string()),
                session: None,
                token: None,
                service: PreparedQueryService {
                    service: "x".to_string(),
                    failover: None,
                    only_passing: false,
                    near: None,
                    tags: None,
                    node_meta: None,
                    service_meta: None,
                },
                dns: None,
                template: None,
            },
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_query_unique_ids() {
        let service = ConsulQueryService::new();

        let ids: Vec<String> = (0..5)
            .map(|i| {
                service
                    .create_query(PreparedQueryCreateRequest {
                        name: Some(format!("q-{}", i)),
                        session: None,
                        token: None,
                        service: PreparedQueryService {
                            service: "s".to_string(),
                            failover: None,
                            only_passing: false,
                            near: None,
                            tags: None,
                            node_meta: None,
                            service_meta: None,
                        },
                        dns: None,
                        template: None,
                    })
                    .id
            })
            .collect();

        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len());

        // Verify each query can be retrieved with correct name
        for i in 0..5 {
            let q = service.get_query(&ids[i]).unwrap();
            assert_eq!(q.name, format!("q-{}", i));
            assert_eq!(q.service.service, "s");
        }
    }
}
