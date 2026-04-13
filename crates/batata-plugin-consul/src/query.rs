// Consul Prepared Query API handlers
// POST /v1/query - Create a prepared query
// GET /v1/query - List all prepared queries
// GET /v1/query/{uuid} - Get a prepared query
// PUT /v1/query/{uuid} - Update a prepared query
// DELETE /v1/query/{uuid} - Delete a prepared query
// GET /v1/query/{uuid}/execute - Execute a prepared query
// GET /v1/query/{uuid}/explain - Explain a prepared query

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use rocksdb::DB;
use tracing::{error, info};

use crate::constants::CF_CONSUL_QUERIES;
use crate::raft::ConsulRaftRequest;

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{
    AgentService, AgentServiceRegistration, ConsulDatacenterConfig, ConsulError, HealthCheck, Node,
    PreparedQuery, PreparedQueryCreateRequest, PreparedQueryCreateResponse, PreparedQueryDNS,
    PreparedQueryExecuteResult, PreparedQueryExplainResult, PreparedQueryParams, ServiceHealth,
};
use crate::naming_store::ConsulNamingStore;
use crate::raft::ConsulRaftWriter;

/// Index counter for prepared queries
static QUERY_INDEX: AtomicU64 = AtomicU64::new(1);

/// Storage backend for prepared queries.
///
/// - `Memory`: instance-level DashMap (for tests and standalone mode)
/// - `Persistent`: RocksDB as single source of truth
#[derive(Clone)]
enum QueryStore {
    Memory {
        queries: Arc<DashMap<String, PreparedQuery>>,
    },
    Persistent {
        db: Arc<DB>,
    },
}

impl QueryStore {
    fn memory() -> Self {
        Self::Memory {
            queries: Arc::new(DashMap::new()),
        }
    }

    fn persistent(db: Arc<DB>) -> Self {
        Self::Persistent { db }
    }

    fn get(&self, id: &str) -> Option<PreparedQuery> {
        match self {
            Self::Memory { queries } => queries.get(id).map(|e| e.value().clone()),
            Self::Persistent { db } => {
                let cf = db.cf_handle(CF_CONSUL_QUERIES)?;
                let bytes = db.get_cf(cf, id.as_bytes()).ok()??;
                serde_json::from_slice(&bytes).ok()
            }
        }
    }

    fn get_by_name(&self, name: &str) -> Option<PreparedQuery> {
        match self {
            Self::Memory { queries } => queries
                .iter()
                .find(|e| e.value().name == name)
                .map(|e| e.value().clone()),
            Self::Persistent { .. } => {
                // Scan all queries for name match
                self.list().into_iter().find(|q| q.name == name)
            }
        }
    }

    fn put(&self, query: &PreparedQuery) {
        match self {
            Self::Memory { queries } => {
                queries.insert(query.id.clone(), query.clone());
            }
            Self::Persistent { db } => {
                if let Some(cf) = db.cf_handle(CF_CONSUL_QUERIES) {
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
        }
    }

    fn remove(&self, id: &str) -> bool {
        match self {
            Self::Memory { queries } => queries.remove(id).is_some(),
            Self::Persistent { db } => {
                if let Some(cf) = db.cf_handle(CF_CONSUL_QUERIES) {
                    let existed = db.get_cf(cf, id.as_bytes()).ok().flatten().is_some();
                    if existed {
                        if let Err(e) = db.delete_cf(cf, id.as_bytes()) {
                            error!("Failed to delete query '{}': {}", id, e);
                        }
                    }
                    existed
                } else {
                    false
                }
            }
        }
    }

    fn list(&self) -> Vec<PreparedQuery> {
        match self {
            Self::Memory { queries } => queries.iter().map(|e| e.value().clone()).collect(),
            Self::Persistent { db } => {
                let cf = match db.cf_handle(CF_CONSUL_QUERIES) {
                    Some(cf) => cf,
                    None => return vec![],
                };
                let mut results = Vec::new();
                let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                for item in iter.flatten() {
                    let (_, value_bytes) = item;
                    if let Ok(query) = serde_json::from_slice::<PreparedQuery>(&value_bytes) {
                        results.push(query);
                    }
                }
                results
            }
        }
    }
}

/// Prepared query service.
///
/// Storage is handled by `QueryStore`:
/// - `QueryStore::Memory`: instance-level DashMap (no global state)
/// - `QueryStore::Persistent`: RocksDB as single source of truth
#[derive(Clone)]
pub struct ConsulQueryService {
    /// Storage backend
    store: QueryStore,
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
            store: QueryStore::memory(),
            raft_node: None,
        }
    }

    /// Create a new query service with RocksDB as single source of truth.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        let store = QueryStore::persistent(db);

        // Sync QUERY_INDEX from stored data
        let mut max_index = 0u64;
        for query in store.list() {
            if let Some(idx) = query.modify_index {
                if idx > max_index {
                    max_index = idx;
                }
            }
        }
        if max_index > 0 {
            let current = QUERY_INDEX.load(Ordering::SeqCst);
            if max_index + 1 > current {
                QUERY_INDEX.store(max_index + 1, Ordering::SeqCst);
            }
        }

        let count = store.list().len();
        if count > 0 {
            info!(
                "Query store initialized (RocksDB): {} prepared queries",
                count
            );
        }

        Self {
            store,
            raft_node: None,
        }
    }

    /// Create a query service with Raft-replicated storage (cluster mode).
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<ConsulRaftWriter>) -> Self {
        let mut svc = Self::with_rocks(db);
        svc.raft_node = Some(raft_node);
        svc
    }

    /// Create a new prepared query
    pub async fn create_query(&self, request: PreparedQueryCreateRequest) -> PreparedQuery {
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

        self.store.put(&query);

        if let Some(ref raft) = self.raft_node {
            let query_json = serde_json::to_string(&query).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::QueryCreate { id, query_json })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft QueryCreate rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft QueryCreate failed: {}", e);
                }
                _ => {}
            }
        }

        query
    }

    /// Get a prepared query by ID or name
    pub fn get_query(&self, id_or_name: &str) -> Option<PreparedQuery> {
        // Try by ID first
        if let Some(query) = self.store.get(id_or_name) {
            return Some(query);
        }
        // Try by name
        self.store.get_by_name(id_or_name)
    }

    /// Update a prepared query
    pub async fn update_query(
        &self,
        id: &str,
        request: PreparedQueryCreateRequest,
    ) -> Option<PreparedQuery> {
        let mut query = self.store.get(id)?;
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

        self.store.put(&query);

        if let Some(ref raft) = self.raft_node {
            let query_json = serde_json::to_string(&query).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::QueryUpdate {
                    id: id.to_string(),
                    query_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft QueryUpdate rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft QueryUpdate failed: {}", e);
                }
                _ => {}
            }
        }

        Some(query)
    }

    /// Delete a prepared query
    pub async fn delete_query(&self, id: &str) -> bool {
        let removed = self.store.remove(id);
        if removed {
            if let Some(ref raft) = self.raft_node {
                match raft
                    .write(ConsulRaftRequest::QueryDelete { id: id.to_string() })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft QueryDelete rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft QueryDelete failed: {}", e);
                    }
                    _ => {}
                }
            }
        }
        removed
    }

    /// List all prepared queries
    pub fn list_queries(&self) -> Vec<PreparedQuery> {
        self.store.list()
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

    let query = query_service.create_query(body.into_inner()).await;

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Queries));
    consul_ok(&meta).json(PreparedQueryCreateResponse { id: query.id })
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

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Queries));
    consul_ok(&meta).json(queries)
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
        Some(query) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Queries));
            consul_ok(&meta).json(vec![query])
        }
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

    match query_service.update_query(&id, body.into_inner()).await {
        Some(_) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Queries));
            consul_ok(&meta).finish()
        }
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

    if query_service.delete_query(&id).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Queries));
        consul_ok(&meta).finish()
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

        let node_id = dc_config.node_id.clone();
        let node_name = dc_config.node_name.clone();

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
                create_index: 1,
                modify_index: 1,
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
                service_tags: reg.tags.clone().unwrap_or_default(),
                check_type: "ttl".to_string(),
                interval: None,
                timeout: None,
                exposed_port: 0,
                create_index: 1,
                modify_index: 1,
                definition: None,
            }],
        });
    }

    // Handle failover: if no healthy nodes found and failover is configured,
    // try alternative namespaces that might have the same service.
    // Cross-datacenter failover requires multi-DC support (not yet implemented).
    let failover_count = if nodes.is_empty() {
        if let Some(ref failover) = query.service.failover {
            let mut attempted = 0i32;

            // Try alternate namespaces in the same DC as a basic failover
            // (Consul's failover.NearestN tries N nearest datacenters)
            if let Some(n) = failover.nearest_n {
                attempted += n;
            }
            if let Some(ref dcs) = failover.datacenters {
                attempted += dcs.len() as i32;
                // In single-DC mode, try loading from alternate namespaces
                // named after the datacenter (best-effort compatibility)
                for alt_dc in dcs {
                    let entries =
                        naming_store.get_service_entries(alt_dc, &service_name);
                    for entry_bytes in &entries {
                        if let Ok(reg) =
                            serde_json::from_slice::<AgentServiceRegistration>(entry_bytes)
                        {
                            let healthy = naming_store.is_healthy(
                                &reg.effective_address(),
                                reg.effective_port() as i32,
                            );
                            if healthy || !query.service.only_passing {
                                let agent_service = AgentService::from(&reg);
                                let ip = reg.effective_address();
                                let node_name = format!("node-{}", ip.replace('.', "-"));
                                nodes.push(ServiceHealth {
                                    node: Node {
                                        id: String::new(),
                                        node: node_name,
                                        address: ip,
                                        datacenter: alt_dc.clone(),
                                        tagged_addresses: None,
                                        meta: None,
                                        create_index: 1,
                                        modify_index: 1,
                                    },
                                    service: agent_service,
                                    checks: vec![HealthCheck {
                                        node: String::new(),
                                        check_id: "serfHealth".to_string(),
                                        name: "Serf Health Status".to_string(),
                                        status: if healthy { "passing" } else { "critical" }.to_string(),
                                        ..Default::default()
                                    }],
                                });
                            }
                        }
                    }
                    if !nodes.is_empty() {
                        tracing::info!(
                            "Prepared query '{}' failover succeeded from dc '{}'",
                            query.name,
                            alt_dc
                        );
                        break;
                    }
                }
            }

            if nodes.is_empty() && attempted > 0 {
                tracing::debug!(
                    "Prepared query '{}' returned 0 nodes after {} failover attempts",
                    query.name,
                    attempted
                );
            }

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

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Queries));
    consul_ok(&meta).json(result)
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
        Some(query) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Queries));
            consul_ok(&meta).json(PreparedQueryExplainResult { query })
        }
        None => HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PreparedQueryService;

    #[tokio::test]
    async fn test_create_query() {
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

        let query = service.create_query(request).await;
        assert_eq!(query.name, "my-query");
        assert_eq!(query.service.service, "web");
        assert!(query.service.only_passing);
    }

    #[tokio::test]
    async fn test_get_query() {
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

        let created = service.create_query(request).await;
        let fetched = service.get_query(&created.id);

        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "test-query");
        assert_eq!(fetched.service.service, "api");
        assert!(!fetched.service.only_passing);
        assert!(fetched.service.tags.is_none());
    }

    #[tokio::test]
    async fn test_delete_query() {
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

        let created = service.create_query(request).await;
        assert!(service.delete_query(&created.id).await);
        assert!(service.get_query(&created.id).is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_query() {
        let service = ConsulQueryService::new();
        assert!(!service.delete_query("nonexistent-id").await);
    }

    #[tokio::test]
    async fn test_list_queries() {
        let service = ConsulQueryService::new();

        // Create queries with unique names
        let unique = uuid::Uuid::new_v4().to_string();
        let mut created_ids = Vec::new();
        for i in 0..3 {
            let q = service
                .create_query(PreparedQueryCreateRequest {
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
                })
                .await;
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

    #[tokio::test]
    async fn test_update_query() {
        let service = ConsulQueryService::new();

        let created = service
            .create_query(PreparedQueryCreateRequest {
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
            })
            .await;

        let updated = service
            .update_query(
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
            )
            .await;

        assert!(updated.is_some());
        let q = updated.unwrap();
        assert_eq!(q.name, "updated");
        assert_eq!(q.service.service, "api");
        assert!(q.service.only_passing);
        assert_eq!(q.id, created.id); // ID preserved
    }

    #[tokio::test]
    async fn test_update_nonexistent_query() {
        let service = ConsulQueryService::new();

        let result = service
            .update_query(
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
            )
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_query_unique_ids() {
        let service = ConsulQueryService::new();

        let mut ids = Vec::new();
        for i in 0..5 {
            let q = service
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
                .await;
            ids.push(q.id);
        }

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
