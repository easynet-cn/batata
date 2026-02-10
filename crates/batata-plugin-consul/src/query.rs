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

use batata_consistency::raft::state_machine::CF_CONSUL_QUERIES;
use batata_naming::service::NamingService;

use crate::acl::{AclService, ResourceType};
use crate::model::{
    AgentService, ConsulError, HealthCheck, Node, PreparedQuery, PreparedQueryCreateRequest,
    PreparedQueryCreateResponse, PreparedQueryDNS, PreparedQueryExecuteResult,
    PreparedQueryExplainResult, PreparedQueryParams, ServiceHealth, Weights,
};

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
}

impl Default for ConsulQueryService {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsulQueryService {
    pub fn new() -> Self {
        Self { rocks_db: None }
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

        Self { rocks_db: Some(db) }
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
) -> HttpResponse {
    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let query = query_service.create_query(body.into_inner());

    HttpResponse::Ok().json(PreparedQueryCreateResponse { id: query.id })
}

/// GET /v1/query
/// List all prepared queries
pub async fn list_queries(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
) -> HttpResponse {
    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let queries = query_service.list_queries();

    HttpResponse::Ok().json(queries)
}

/// GET /v1/query/{uuid}
/// Get a prepared query by UUID
pub async fn get_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.get_query(&id) {
        Some(query) => HttpResponse::Ok().json(vec![query]),
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
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.update_query(&id, body.into_inner()) {
        Some(_) => HttpResponse::Ok().finish(),
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
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    if query_service.delete_query(&id) {
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::NotFound().json(ConsulError::new("Query not found"))
    }
}

/// GET /v1/query/{uuid}/execute
/// Execute a prepared query
pub async fn execute_query(
    req: HttpRequest,
    path: web::Path<String>,
    _query_params: web::Query<PreparedQueryParams>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    naming_service: web::Data<Arc<NamingService>>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let query = match query_service.get_query(&id) {
        Some(q) => q,
        None => return HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    };

    // Execute the query by fetching service instances
    let service_name = &query.service.service;
    let namespace = "public";
    let only_passing = query.service.only_passing;

    let instances =
        naming_service.get_instances(namespace, "DEFAULT_GROUP", service_name, "", only_passing);

    // Convert to ServiceHealth format
    let nodes: Vec<ServiceHealth> = instances
        .iter()
        .map(|instance| {
            let tags: Option<Vec<String>> = instance
                .metadata
                .get("consul_tags")
                .and_then(|s| serde_json::from_str(s).ok());

            ServiceHealth {
                node: Node {
                    id: uuid::Uuid::new_v4().to_string(),
                    node: hostname::get()
                        .map(|h| h.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "batata-node".to_string()),
                    address: instance.ip.clone(),
                    datacenter: "dc1".to_string(),
                    tagged_addresses: None,
                    meta: None,
                },
                service: AgentService {
                    id: instance.instance_id.clone(),
                    service: instance.service_name.clone(),
                    tags: tags.clone(),
                    port: instance.port as u16,
                    address: instance.ip.clone(),
                    meta: Some(instance.metadata.clone()),
                    enable_tag_override: false,
                    weights: Weights {
                        passing: instance.weight as i32,
                        warning: 1,
                    },
                    datacenter: Some("dc1".to_string()),
                    namespace: Some(namespace.to_string()),
                    kind: instance.metadata.get("consul_kind").cloned(),
                    proxy: instance
                        .metadata
                        .get("consul_proxy")
                        .and_then(|s| serde_json::from_str(s).ok()),
                    connect: instance
                        .metadata
                        .get("consul_connect")
                        .and_then(|s| serde_json::from_str(s).ok()),
                    tagged_addresses: instance
                        .metadata
                        .get("consul_tagged_addresses")
                        .and_then(|s| serde_json::from_str(s).ok()),
                },
                checks: vec![HealthCheck {
                    node: "batata-node".to_string(),
                    check_id: format!("service:{}", instance.instance_id),
                    name: "Service health check".to_string(),
                    status: if instance.healthy {
                        "passing"
                    } else {
                        "critical"
                    }
                    .to_string(),
                    notes: String::new(),
                    output: String::new(),
                    service_id: instance.instance_id.clone(),
                    service_name: instance.service_name.clone(),
                    service_tags: tags,
                    check_type: "ttl".to_string(),
                    create_index: None,
                    modify_index: None,
                }],
            }
        })
        .collect();

    let result = PreparedQueryExecuteResult {
        service: service_name.clone(),
        nodes,
        dns: query.dns.unwrap_or(PreparedQueryDNS { ttl: None }),
        datacenter: "dc1".to_string(),
        failovers: 0,
    };

    HttpResponse::Ok().json(result)
}

/// GET /v1/query/{uuid}/explain
/// Explain a prepared query
pub async fn explain_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.get_query(&id) {
        Some(query) => HttpResponse::Ok().json(PreparedQueryExplainResult { query }),
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
        assert_eq!(fetched.unwrap().id, created.id);
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
    }
}
