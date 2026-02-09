// Consul Event API handlers
// PUT /v1/event/fire/{name} - Fire a new user event
// GET /v1/event/list - List recent events

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use sea_orm::DatabaseConnection;

use crate::acl::{AclService, ResourceType};
use crate::model::{ConsulError, EventFireParams, EventFireRequest, EventListParams, UserEvent};

// ConfigService storage constants for events
const CONSUL_EVENT_NAMESPACE: &str = "public";
const CONSUL_EVENT_GROUP: &str = "consul-events";

/// Global event storage
static EVENTS: LazyLock<DashMap<String, UserEvent>> = LazyLock::new(DashMap::new);

/// Event counter for LTime (Lamport time)
static EVENT_LTIME: AtomicU64 = AtomicU64::new(1);

/// Maximum events to store
const MAX_EVENTS: usize = 256;

/// Event service for managing user events
#[derive(Clone, Default)]
pub struct ConsulEventService;

impl ConsulEventService {
    pub fn new() -> Self {
        Self
    }

    /// Fire a new user event
    pub fn fire_event(
        &self,
        name: &str,
        payload: Option<String>,
        node_filter: &str,
        service_filter: &str,
        tag_filter: &str,
    ) -> UserEvent {
        let id = uuid::Uuid::new_v4().to_string();
        let ltime = EVENT_LTIME.fetch_add(1, Ordering::SeqCst);

        let event = UserEvent {
            id: id.clone(),
            name: name.to_string(),
            payload,
            node_filter: node_filter.to_string(),
            service_filter: service_filter.to_string(),
            tag_filter: tag_filter.to_string(),
            version: 1,
            ltime,
        };

        // Store event (with size limit)
        if EVENTS.len() >= MAX_EVENTS {
            // Remove oldest event by LTime
            let oldest: Option<String> = EVENTS
                .iter()
                .min_by_key(|e| e.value().ltime)
                .map(|e| e.key().clone());
            if let Some(key) = oldest {
                EVENTS.remove(&key);
            }
        }

        EVENTS.insert(id, event.clone());
        event
    }

    /// List events with optional filters
    pub fn list_events(
        &self,
        name: Option<&str>,
        node: Option<&str>,
        service: Option<&str>,
        tag: Option<&str>,
    ) -> Vec<UserEvent> {
        let mut events: Vec<UserEvent> = EVENTS
            .iter()
            .filter(|e| {
                // Filter by name
                if let Some(n) = name
                    && e.value().name != n
                {
                    return false;
                }

                // Filter by node (regex match on node_filter)
                if let Some(n) = node
                    && !e.value().node_filter.is_empty()
                    && !e.value().node_filter.contains(n)
                {
                    return false;
                }

                // Filter by service (regex match on service_filter)
                if let Some(s) = service
                    && !e.value().service_filter.is_empty()
                    && !e.value().service_filter.contains(s)
                {
                    return false;
                }

                // Filter by tag (regex match on tag_filter)
                if let Some(t) = tag
                    && !e.value().tag_filter.is_empty()
                    && !e.value().tag_filter.contains(t)
                {
                    return false;
                }

                true
            })
            .map(|e| e.value().clone())
            .collect();

        // Sort by LTime (oldest first)
        events.sort_by_key(|e| e.ltime);
        events
    }
}

// ============================================================================
// Persistent Event Service (Database-backed via ConfigService)
// ============================================================================

/// Event service with persistent storage via ConfigService
/// Events are stored as config items and cached in memory
#[derive(Clone)]
pub struct ConsulEventServicePersistent {
    db: Arc<DatabaseConnection>,
    cache: Arc<DashMap<String, UserEvent>>,
    ltime: Arc<AtomicU64>,
    initialized: Arc<std::sync::atomic::AtomicBool>,
}

impl ConsulEventServicePersistent {
    /// Create a new persistent event service
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            cache: Arc::new(DashMap::new()),
            ltime: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn event_data_id(event_id: &str) -> String {
        format!("event:{}", event_id.replace('/', ":"))
    }

    /// Initialize cache from database
    async fn ensure_initialized(&self) {
        if self.initialized.swap(true, Ordering::SeqCst) {
            return;
        }

        // Load events from database using search_page
        if let Ok(page) = batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_EVENT_NAMESPACE,
            "event:*",
            CONSUL_EVENT_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            let mut max_ltime = 0u64;
            for info in page.page_items {
                // Load the full config to get the content
                if let Ok(Some(config)) = batata_config::service::config::find_one(
                    &self.db,
                    &info.data_id,
                    CONSUL_EVENT_GROUP,
                    CONSUL_EVENT_NAMESPACE,
                )
                .await
                    && let Ok(event) = serde_json::from_str::<UserEvent>(
                        &config.config_info.config_info_base.content,
                    )
                {
                    if event.ltime > max_ltime {
                        max_ltime = event.ltime;
                    }
                    self.cache.insert(event.id.clone(), event);
                }
            }
            // Set ltime to be greater than max found
            if max_ltime > 0 {
                self.ltime.store(max_ltime + 1, Ordering::SeqCst);
            }
        }
    }

    async fn save_event(&self, event: &UserEvent) -> Result<(), String> {
        let content =
            serde_json::to_string(event).map_err(|e| format!("Serialization error: {}", e))?;
        let data_id = Self::event_data_id(&event.id);

        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_EVENT_GROUP,
            CONSUL_EVENT_NAMESPACE,
            &content,
            "consul-event",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul Event: {}", event.name),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        self.cache.insert(event.id.clone(), event.clone());
        Ok(())
    }

    async fn delete_event_from_db(&self, event_id: &str) -> bool {
        let data_id = Self::event_data_id(event_id);
        self.cache.remove(event_id);
        batata_config::service::config::delete(
            &self.db,
            &data_id,
            CONSUL_EVENT_GROUP,
            CONSUL_EVENT_NAMESPACE,
            "",
            "127.0.0.1",
            "system",
        )
        .await
        .is_ok()
    }

    /// Fire a new user event (with persistence)
    pub async fn fire_event(
        &self,
        name: &str,
        payload: Option<String>,
        node_filter: &str,
        service_filter: &str,
        tag_filter: &str,
    ) -> UserEvent {
        self.ensure_initialized().await;

        let id = uuid::Uuid::new_v4().to_string();
        let ltime = self.ltime.fetch_add(1, Ordering::SeqCst);

        let event = UserEvent {
            id: id.clone(),
            name: name.to_string(),
            payload,
            node_filter: node_filter.to_string(),
            service_filter: service_filter.to_string(),
            tag_filter: tag_filter.to_string(),
            version: 1,
            ltime,
        };

        // Enforce size limit (remove oldest events)
        if self.cache.len() >= MAX_EVENTS {
            let oldest: Option<String> = self
                .cache
                .iter()
                .min_by_key(|e| e.value().ltime)
                .map(|e| e.key().clone());
            if let Some(old_id) = oldest {
                let _ = self.delete_event_from_db(&old_id).await;
            }
        }

        // Persist to database
        let _ = self.save_event(&event).await;

        event
    }

    /// List events with optional filters (from cache)
    pub async fn list_events(
        &self,
        name: Option<&str>,
        node: Option<&str>,
        service: Option<&str>,
        tag: Option<&str>,
    ) -> Vec<UserEvent> {
        self.ensure_initialized().await;

        let mut events: Vec<UserEvent> = self
            .cache
            .iter()
            .filter(|e| {
                // Filter by name
                if let Some(n) = name
                    && e.value().name != n
                {
                    return false;
                }

                // Filter by node (regex match on node_filter)
                if let Some(n) = node
                    && !e.value().node_filter.is_empty()
                    && !e.value().node_filter.contains(n)
                {
                    return false;
                }

                // Filter by service (regex match on service_filter)
                if let Some(s) = service
                    && !e.value().service_filter.is_empty()
                    && !e.value().service_filter.contains(s)
                {
                    return false;
                }

                // Filter by tag (regex match on tag_filter)
                if let Some(t) = tag
                    && !e.value().tag_filter.is_empty()
                    && !e.value().tag_filter.contains(t)
                {
                    return false;
                }

                true
            })
            .map(|e| e.value().clone())
            .collect();

        // Sort by LTime (oldest first)
        events.sort_by_key(|e| e.ltime);
        events
    }
}

/// PUT /v1/event/fire/{name}
/// Fire a new user event to the cluster
pub async fn fire_event(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<EventFireParams>,
    body: Option<web::Json<EventFireRequest>>,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    let name = path.into_inner();

    // Check ACL authorization for event write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &name, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let payload = body.and_then(|b| b.payload.clone());
    let node_filter = query.node.clone().unwrap_or_default();
    let service_filter = query.service.clone().unwrap_or_default();
    let tag_filter = query.tag.clone().unwrap_or_default();

    let service = ConsulEventService::new();
    let event = service.fire_event(&name, payload, &node_filter, &service_filter, &tag_filter);

    HttpResponse::Ok().json(event)
}

/// GET /v1/event/list
/// List the most recent events
pub async fn list_events(
    req: HttpRequest,
    query: web::Query<EventListParams>,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    // Check ACL authorization for event read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service = ConsulEventService::new();
    let events = service.list_events(
        query.name.as_deref(),
        query.node.as_deref(),
        query.service.as_deref(),
        query.tag.as_deref(),
    );

    HttpResponse::Ok().json(events)
}

// ============================================================================
// Persistent Event API Endpoints (Using ConfigService)
// ============================================================================

/// PUT /v1/event/fire/{name} (Persistent)
/// Fire a new user event to the cluster with database persistence
pub async fn fire_event_persistent(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<EventFireParams>,
    body: Option<web::Json<EventFireRequest>>,
    acl_service: web::Data<AclService>,
    event_service: web::Data<ConsulEventServicePersistent>,
) -> HttpResponse {
    let name = path.into_inner();

    // Check ACL authorization for event write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &name, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let payload = body.and_then(|b| b.payload.clone());
    let node_filter = query.node.clone().unwrap_or_default();
    let service_filter = query.service.clone().unwrap_or_default();
    let tag_filter = query.tag.clone().unwrap_or_default();

    let event = event_service
        .fire_event(&name, payload, &node_filter, &service_filter, &tag_filter)
        .await;

    HttpResponse::Ok().json(event)
}

/// GET /v1/event/list (Persistent)
/// List the most recent events from database
pub async fn list_events_persistent(
    req: HttpRequest,
    query: web::Query<EventListParams>,
    acl_service: web::Data<AclService>,
    event_service: web::Data<ConsulEventServicePersistent>,
) -> HttpResponse {
    // Check ACL authorization for event read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let events = event_service
        .list_events(
            query.name.as_deref(),
            query.node.as_deref(),
            query.service.as_deref(),
            query.tag.as_deref(),
        )
        .await;

    HttpResponse::Ok().json(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fire_event() {
        let service = ConsulEventService::new();

        let event = service.fire_event("deploy", Some("v1.0".to_string()), "", "web", "");

        assert_eq!(event.name, "deploy");
        assert_eq!(event.payload, Some("v1.0".to_string()));
        assert_eq!(event.service_filter, "web");
    }

    #[test]
    fn test_list_events() {
        let service = ConsulEventService::new();

        service.fire_event("event1", None, "", "", "");
        service.fire_event("event2", None, "", "", "");

        let events = service.list_events(None, None, None, None);
        assert!(events.len() >= 2);
    }

    #[test]
    fn test_list_events_filter_by_name() {
        let service = ConsulEventService::new();

        service.fire_event("specific-event", None, "", "", "");
        service.fire_event("other-event", None, "", "", "");

        let events = service.list_events(Some("specific-event"), None, None, None);
        assert!(events.iter().all(|e| e.name == "specific-event"));
    }

    #[test]
    fn test_event_unique_ids() {
        let service = ConsulEventService::new();

        let e1 = service.fire_event("evt", None, "", "", "");
        let e2 = service.fire_event("evt", None, "", "", "");
        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn test_event_with_all_filters() {
        let service = ConsulEventService::new();

        // Use unique filter values to avoid interference from other tests
        service.fire_event(
            "filter-deploy",
            None,
            "filter-node-1",
            "filter-web",
            "filter-v1",
        );
        service.fire_event(
            "filter-deploy",
            None,
            "filter-node-2",
            "filter-api",
            "filter-v2",
        );
        service.fire_event(
            "filter-restart",
            None,
            "filter-node-1",
            "filter-web",
            "filter-v1",
        );

        // Filter by node — events with empty node_filter also pass through (match-all semantics)
        let by_node = service.list_events(None, Some("filter-node-1"), None, None);
        assert!(by_node.len() >= 2);
        assert!(
            by_node
                .iter()
                .all(|e| e.node_filter.is_empty() || e.node_filter.contains("filter-node-1"))
        );

        // Filter by service — events with empty service_filter also pass through
        let by_svc = service.list_events(None, None, Some("filter-web"), None);
        assert!(by_svc.len() >= 2);
        assert!(
            by_svc
                .iter()
                .all(|e| e.service_filter.is_empty() || e.service_filter.contains("filter-web"))
        );

        // Filter by tag — events with empty tag_filter also pass through
        let by_tag = service.list_events(None, None, None, Some("filter-v2"));
        assert!(by_tag.len() >= 1);
        assert!(
            by_tag
                .iter()
                .all(|e| e.tag_filter.is_empty() || e.tag_filter.contains("filter-v2"))
        );

        // Combined filters
        let combined =
            service.list_events(Some("filter-deploy"), Some("filter-node-1"), None, None);
        assert!(combined.len() >= 1);
        assert!(combined.iter().all(|e| e.name == "filter-deploy"
            && (e.node_filter.is_empty() || e.node_filter.contains("filter-node-1"))));
    }

    #[test]
    fn test_event_empty_payload() {
        let service = ConsulEventService::new();

        let event = service.fire_event("no-payload-evt", None, "", "", "");
        assert!(event.payload.is_none());
    }

    #[test]
    fn test_event_ltime_increments() {
        let service = ConsulEventService::new();

        let e1 = service.fire_event("ltime-a", None, "", "", "");
        let e2 = service.fire_event("ltime-b", None, "", "", "");
        assert!(e2.ltime > e1.ltime);
    }
}
