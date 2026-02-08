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
use sea_orm::DatabaseConnection;

use batata_naming::service::NamingService;

use crate::acl::{AclService, ResourceType};
use crate::model::{
    AgentService, ConsulError, HealthCheck, Node, PreparedQuery, PreparedQueryCreateRequest,
    PreparedQueryCreateResponse, PreparedQueryDNS, PreparedQueryExecuteResult,
    PreparedQueryExplainResult, PreparedQueryParams, ServiceHealth, Weights,
};

// ConfigService storage constants for prepared queries
const CONSUL_QUERY_NAMESPACE: &str = "public";
const CONSUL_QUERY_GROUP: &str = "consul-queries";

/// Global prepared query storage
static QUERIES: LazyLock<DashMap<String, PreparedQuery>> = LazyLock::new(DashMap::new);

/// Index counter for prepared queries
static QUERY_INDEX: AtomicU64 = AtomicU64::new(1);

/// Prepared query service
#[derive(Clone, Default)]
pub struct ConsulQueryService;

impl ConsulQueryService {
    pub fn new() -> Self {
        Self
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

        Some(query.clone())
    }

    /// Delete a prepared query
    pub fn delete_query(&self, id: &str) -> bool {
        QUERIES.remove(id).is_some()
    }

    /// List all prepared queries
    pub fn list_queries(&self) -> Vec<PreparedQuery> {
        QUERIES.iter().map(|entry| entry.value().clone()).collect()
    }
}

// ============================================================================
// Persistent Prepared Query Service (Database-backed via ConfigService)
// ============================================================================

/// Prepared query service with persistent storage via ConfigService
#[derive(Clone)]
pub struct ConsulQueryServicePersistent {
    db: Arc<DatabaseConnection>,
    cache: Arc<DashMap<String, PreparedQuery>>,
    index: Arc<AtomicU64>,
    initialized: Arc<std::sync::atomic::AtomicBool>,
}

impl ConsulQueryServicePersistent {
    /// Create a new persistent query service
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            cache: Arc::new(DashMap::new()),
            index: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn query_data_id(query_id: &str) -> String {
        format!("query:{}", query_id.replace('/', ":"))
    }

    fn next_index(&self) -> u64 {
        self.index.fetch_add(1, Ordering::SeqCst)
    }

    /// Initialize cache from database
    async fn ensure_initialized(&self) {
        if self.initialized.swap(true, Ordering::SeqCst) {
            return;
        }

        // Load queries from database
        if let Ok(page) = batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_QUERY_NAMESPACE,
            "query:*",
            CONSUL_QUERY_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            let mut max_index = 0u64;
            for info in page.page_items {
                if let Ok(Some(config)) = batata_config::service::config::find_one(
                    &self.db,
                    &info.data_id,
                    CONSUL_QUERY_GROUP,
                    CONSUL_QUERY_NAMESPACE,
                )
                .await
                    && let Ok(query) = serde_json::from_str::<PreparedQuery>(
                        &config.config_info.config_info_base.content,
                    )
                {
                    if let Some(idx) = query.modify_index
                        && idx > max_index
                    {
                        max_index = idx;
                    }
                    self.cache.insert(query.id.clone(), query);
                }
            }
            // Set index to be greater than max found
            if max_index > 0 {
                self.index.store(max_index + 1, Ordering::SeqCst);
            }
        }
    }

    async fn save_query(&self, query: &PreparedQuery) -> Result<(), String> {
        let content =
            serde_json::to_string(query).map_err(|e| format!("Serialization error: {}", e))?;
        let data_id = Self::query_data_id(&query.id);

        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_QUERY_GROUP,
            CONSUL_QUERY_NAMESPACE,
            &content,
            "consul-query",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul Prepared Query: {}", query.name),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        self.cache.insert(query.id.clone(), query.clone());
        Ok(())
    }

    async fn delete_query_from_db(&self, query_id: &str) -> bool {
        let data_id = Self::query_data_id(query_id);
        self.cache.remove(query_id);
        batata_config::service::config::delete(
            &self.db,
            &data_id,
            CONSUL_QUERY_GROUP,
            CONSUL_QUERY_NAMESPACE,
            "",
            "127.0.0.1",
            "system",
        )
        .await
        .is_ok()
    }

    /// Create a new prepared query (with persistence)
    pub async fn create_query(&self, request: PreparedQueryCreateRequest) -> PreparedQuery {
        self.ensure_initialized().await;

        let id = uuid::Uuid::new_v4().to_string();
        let index = self.next_index();

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

        let _ = self.save_query(&query).await;
        query
    }

    /// Get a prepared query by ID or name
    pub async fn get_query(&self, id_or_name: &str) -> Option<PreparedQuery> {
        self.ensure_initialized().await;

        // Try by ID first
        if let Some(query) = self.cache.get(id_or_name) {
            return Some(query.clone());
        }

        // Try by name
        self.cache
            .iter()
            .find(|entry| entry.value().name == id_or_name)
            .map(|entry| entry.value().clone())
    }

    /// Update a prepared query
    pub async fn update_query(
        &self,
        id: &str,
        request: PreparedQueryCreateRequest,
    ) -> Option<PreparedQuery> {
        self.ensure_initialized().await;

        let mut query = self.cache.get_mut(id)?;
        let index = self.next_index();

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
        drop(query); // Release the lock before saving

        let _ = self.save_query(&updated).await;
        Some(updated)
    }

    /// Delete a prepared query
    pub async fn delete_query(&self, id: &str) -> bool {
        self.ensure_initialized().await;
        self.delete_query_from_db(id).await
    }

    /// List all prepared queries
    pub async fn list_queries(&self) -> Vec<PreparedQuery> {
        self.ensure_initialized().await;
        self.cache
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
}

/// POST /v1/query
/// Create a new prepared query
pub async fn create_query(
    req: HttpRequest,
    body: web::Json<PreparedQueryCreateRequest>,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service = ConsulQueryService::new();
    let query = service.create_query(body.into_inner());

    HttpResponse::Ok().json(PreparedQueryCreateResponse { id: query.id })
}

/// GET /v1/query
/// List all prepared queries
pub async fn list_queries(req: HttpRequest, acl_service: web::Data<AclService>) -> HttpResponse {
    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service = ConsulQueryService::new();
    let queries = service.list_queries();

    HttpResponse::Ok().json(queries)
}

/// GET /v1/query/{uuid}
/// Get a prepared query by UUID
pub async fn get_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service = ConsulQueryService::new();
    match service.get_query(&id) {
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
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service = ConsulQueryService::new();
    match service.update_query(&id, body.into_inner()) {
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
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service = ConsulQueryService::new();
    if service.delete_query(&id) {
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
    naming_service: web::Data<Arc<NamingService>>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service = ConsulQueryService::new();
    let query = match service.get_query(&id) {
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
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service = ConsulQueryService::new();
    match service.get_query(&id) {
        Some(query) => HttpResponse::Ok().json(PreparedQueryExplainResult { query }),
        None => HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    }
}

// ============================================================================
// Persistent Prepared Query API Endpoints (Using ConfigService)
// ============================================================================

/// POST /v1/query (Persistent)
/// Create a new prepared query with database persistence
pub async fn create_query_persistent(
    req: HttpRequest,
    body: web::Json<PreparedQueryCreateRequest>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryServicePersistent>,
) -> HttpResponse {
    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let query = query_service.create_query(body.into_inner()).await;
    HttpResponse::Ok().json(PreparedQueryCreateResponse { id: query.id })
}

/// GET /v1/query (Persistent)
/// List all prepared queries from database
pub async fn list_queries_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryServicePersistent>,
) -> HttpResponse {
    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let queries = query_service.list_queries().await;
    HttpResponse::Ok().json(queries)
}

/// GET /v1/query/{uuid} (Persistent)
/// Get a prepared query by UUID from database
pub async fn get_query_persistent(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryServicePersistent>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.get_query(&id).await {
        Some(query) => HttpResponse::Ok().json(vec![query]),
        None => HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    }
}

/// PUT /v1/query/{uuid} (Persistent)
/// Update an existing prepared query with database persistence
pub async fn update_query_persistent(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<PreparedQueryCreateRequest>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryServicePersistent>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.update_query(&id, body.into_inner()).await {
        Some(_) => HttpResponse::Ok().finish(),
        None => HttpResponse::NotFound().json(ConsulError::new("Query not found")),
    }
}

/// DELETE /v1/query/{uuid} (Persistent)
/// Delete a prepared query with database persistence
pub async fn delete_query_persistent(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryServicePersistent>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    if query_service.delete_query(&id).await {
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::NotFound().json(ConsulError::new("Query not found"))
    }
}

/// GET /v1/query/{uuid}/execute (Persistent)
/// Execute a prepared query from database
pub async fn execute_query_persistent(
    req: HttpRequest,
    path: web::Path<String>,
    _query_params: web::Query<PreparedQueryParams>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryServicePersistent>,
    naming_service: web::Data<Arc<NamingService>>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let query = match query_service.get_query(&id).await {
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

/// GET /v1/query/{uuid}/explain (Persistent)
/// Explain a prepared query from database
pub async fn explain_query_persistent(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryServicePersistent>,
) -> HttpResponse {
    let id = path.into_inner();

    // Check ACL authorization for query read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match query_service.get_query(&id).await {
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
}
