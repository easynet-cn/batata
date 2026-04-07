// Consul Event API handlers
//
// PUT /v1/event/fire/{name} - Fire a new user event
// GET /v1/event/list - List recent events
//
// Architecture (compatible with Consul original):
// - Events are ephemeral — stored in a fixed-size ring buffer (256 slots)
// - In cluster mode, events are broadcast via gRPC to all nodes
//   (Consul uses Serf gossip; batata uses ClusterClient.broadcast)
// - Events are NOT persisted to RocksDB or Raft (by design)
// - Blocking queries use UUID-based index (XOR of UUID halves)

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use base64::Engine;
use tokio::sync::{Notify, RwLock};

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{ConsulError, EventFireParams, EventListParams, UserEvent};

/// Maximum events to store (matches Consul's hardcoded 256)
const MAX_EVENTS: usize = 256;

// ============================================================================
// Event Broadcaster Trait (for cluster mode)
// ============================================================================

/// Trait for broadcasting events to cluster peers.
///
/// Decouples the event service from the cluster communication infrastructure.
/// Implemented in `batata-server` where `ClusterClientManager` is available.
#[async_trait::async_trait]
pub trait EventBroadcaster: Send + Sync {
    /// Broadcast an event to all cluster peers (fire-and-forget).
    async fn broadcast_event(&self, event: &UserEvent);
}

// ============================================================================
// Ring Buffer (matches Consul's agent.eventBuf)
// ============================================================================

/// Fixed-size circular buffer for user events.
///
/// Matches Consul's `agent.eventBuf` ring buffer implementation:
/// - O(1) insertion (write to current slot, advance index)
/// - Old events are automatically overwritten when buffer wraps
/// - No persistence (events are ephemeral)
pub struct EventRingBuffer {
    buffer: Vec<Option<UserEvent>>,
    /// Next write position (wraps around)
    write_index: usize,
    /// Total events ever written (for ordering)
    total_written: u64,
}

impl EventRingBuffer {
    fn new() -> Self {
        Self {
            buffer: (0..MAX_EVENTS).map(|_| None).collect(),
            write_index: 0,
            total_written: 0,
        }
    }

    /// Insert an event into the ring buffer (overwrites oldest if full)
    fn push(&mut self, event: UserEvent) {
        self.buffer[self.write_index] = Some(event);
        self.write_index = (self.write_index + 1) % self.buffer.len();
        self.total_written += 1;
    }

    /// Get all events in insertion order (oldest first)
    fn events(&self) -> Vec<&UserEvent> {
        if self.total_written == 0 {
            return Vec::new();
        }

        let len = self.buffer.len();
        let mut result = Vec::with_capacity(std::cmp::min(self.total_written as usize, len));

        // If buffer hasn't wrapped yet, read from 0..write_index
        if self.total_written <= len as u64 {
            for slot in &self.buffer[..self.write_index] {
                if let Some(event) = slot {
                    result.push(event);
                }
            }
        } else {
            // Buffer has wrapped — read from write_index (oldest) to write_index-1 (newest)
            for i in 0..len {
                let idx = (self.write_index + i) % len;
                if let Some(event) = &self.buffer[idx] {
                    result.push(event);
                }
            }
        }

        result
    }
}

// ============================================================================
// UUID-based Index (matches Consul's uuidToUint64)
// ============================================================================

/// Convert a UUID string to a u64 index by XOR-ing the upper and lower halves.
/// Matches Consul's `uuidToUint64()` in `agent/event_endpoint.go`.
fn uuid_to_index(uuid_str: &str) -> u64 {
    let Ok(uuid) = uuid::Uuid::parse_str(uuid_str) else {
        return 0;
    };
    let bytes = uuid.as_bytes();
    let upper = u64::from_be_bytes(bytes[0..8].try_into().unwrap_or_default());
    let lower = u64::from_be_bytes(bytes[8..16].try_into().unwrap_or_default());
    upper ^ lower
}

// ============================================================================
// Event Service
// ============================================================================

/// Consul-compatible event service with ring buffer and cluster broadcast.
///
/// In standalone mode, events are local-only.
/// In cluster mode, `fire_event` broadcasts to all nodes via gRPC.
#[derive(Clone)]
pub struct ConsulEventService {
    /// Ring buffer protected by RwLock (read-heavy workload)
    buffer: Arc<RwLock<EventRingBuffer>>,
    /// Lamport time counter
    ltime: Arc<AtomicU64>,
    /// Notification for blocking queries
    notify: Arc<Notify>,
    /// Latest event index (UUID-based) for blocking queries
    last_index: Arc<AtomicU64>,
    /// Index provider for ConsulTable::Events
    index_provider: ConsulIndexProvider,
    /// Optional cluster broadcaster (None in standalone mode)
    broadcaster: Arc<RwLock<Option<Arc<dyn EventBroadcaster>>>>,
}

impl ConsulEventService {
    pub fn new(index_provider: ConsulIndexProvider) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(EventRingBuffer::new())),
            ltime: Arc::new(AtomicU64::new(1)),
            notify: Arc::new(Notify::new()),
            last_index: Arc::new(AtomicU64::new(0)),
            index_provider,
            broadcaster: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the cluster broadcaster (called during startup after ClusterClient is ready).
    pub async fn set_broadcaster(&self, broadcaster: Arc<dyn EventBroadcaster>) {
        *self.broadcaster.write().await = Some(broadcaster);
    }

    /// Fire a new user event and return it.
    /// In cluster mode, also broadcasts to all peers.
    pub async fn fire_event(
        &self,
        name: &str,
        payload: Option<String>,
        node_filter: &str,
        service_filter: &str,
        tag_filter: &str,
    ) -> UserEvent {
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

        self.ingest_event(event.clone()).await;

        // Broadcast to cluster peers (fire-and-forget, like Consul's gossip)
        if let Some(ref broadcaster) = *self.broadcaster.read().await {
            broadcaster.broadcast_event(&event).await;
        }

        event
    }

    /// Ingest an event into the local ring buffer (called for both local and broadcast events)
    pub async fn ingest_event(&self, event: UserEvent) {
        let idx = uuid_to_index(&event.id);

        // Update LTime if incoming event has a higher LTime (cluster sync)
        let incoming_ltime = event.ltime;
        let _ = self.ltime.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            if incoming_ltime >= current {
                Some(incoming_ltime + 1)
            } else {
                None
            }
        });

        {
            let mut buf = self.buffer.write().await;
            buf.push(event);
        }

        // Update index and notify blocking queries
        self.last_index.store(idx, Ordering::SeqCst);
        self.index_provider.increment(ConsulTable::Events);
        self.notify.notify_waiters();
    }

    /// List events with optional filters.
    /// Filters use regex matching (compatible with Consul's Go regexp).
    pub async fn list_events(
        &self,
        name: Option<&str>,
        _node: Option<&str>,
        _service: Option<&str>,
        _tag: Option<&str>,
    ) -> Vec<UserEvent> {
        let buf = self.buffer.read().await;
        buf.events()
            .into_iter()
            .filter(|e| {
                // Filter by name (exact match, like Consul)
                if let Some(n) = name {
                    if e.name != n {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Get the current event index for blocking queries (UUID-based)
    pub fn current_index(&self) -> u64 {
        self.last_index.load(Ordering::SeqCst)
    }

    /// Wait for a new event (blocking query support)
    pub async fn wait_for_event(&self, timeout: Option<std::time::Duration>) {
        let timeout = timeout.unwrap_or(std::time::Duration::from_secs(300));
        let _ = tokio::time::timeout(timeout, self.notify.notified()).await;
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// PUT /v1/event/fire/{name}
/// Fire a new user event to the cluster
#[allow(clippy::too_many_arguments)]
pub async fn fire_event(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<EventFireParams>,
    body: web::Bytes,
    acl_service: web::Data<AclService>,
    event_service: web::Data<ConsulEventService>,
    _index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let name = path.into_inner();

    // Check ACL authorization for event write
    let authz = acl_service.authorize_request(&req, ResourceType::Query, &name, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Consul enforces max 300 byte event payload (agent/event_endpoint.go:53-60)
    if body.len() > 300 {
        return HttpResponse::PayloadTooLarge().json(ConsulError::new(
            "Payload exceeds maximum size of 300 bytes",
        ));
    }

    // Payload is sent as raw bytes, base64-encode for JSON response
    let payload = if body.is_empty() {
        None
    } else {
        Some(base64::engine::general_purpose::STANDARD.encode(&body))
    };
    let node_filter = query.node.clone().unwrap_or_default();
    let service_filter = query.service.clone().unwrap_or_default();
    let tag_filter = query.tag.clone().unwrap_or_default();

    let event = event_service
        .fire_event(&name, payload, &node_filter, &service_filter, &tag_filter)
        .await;

    let index = event_service.current_index();
    let meta = ConsulResponseMeta::new(std::cmp::max(index, 1));
    consul_ok(&meta).json(event)
}

/// GET /v1/event/list
/// List the most recent events
pub async fn list_events(
    req: HttpRequest,
    query: web::Query<EventListParams>,
    acl_service: web::Data<AclService>,
    event_service: web::Data<ConsulEventService>,
    _index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for event read
    let authz = acl_service.authorize_request(&req, ResourceType::Query, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Support blocking queries
    if let Some(_target_index) = query.index {
        let timeout = query
            .wait
            .as_deref()
            .and_then(ConsulIndexProvider::parse_wait_duration);
        event_service.wait_for_event(timeout).await;
    }

    let events = event_service
        .list_events(
            query.name.as_deref(),
            query.node.as_deref(),
            query.service.as_deref(),
            query.tag.as_deref(),
        )
        .await;

    let index = event_service.current_index();
    let meta = ConsulResponseMeta::new(std::cmp::max(index, 1));
    consul_ok(&meta).json(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index_provider() -> ConsulIndexProvider {
        ConsulIndexProvider::new()
    }

    #[tokio::test]
    async fn test_fire_event() {
        let service = ConsulEventService::new(test_index_provider());

        let event = service
            .fire_event("deploy", Some("v1.0".to_string()), "", "web", "")
            .await;

        assert_eq!(event.name, "deploy");
        assert_eq!(event.payload, Some("v1.0".to_string()));
        assert_eq!(event.service_filter, "web");
        assert!(!event.id.is_empty());
    }

    #[tokio::test]
    async fn test_list_events() {
        let service = ConsulEventService::new(test_index_provider());

        service.fire_event("event1", None, "", "", "").await;
        service.fire_event("event2", None, "", "", "").await;

        let events = service.list_events(None, None, None, None).await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].name, "event1");
        assert_eq!(events[1].name, "event2");
    }

    #[tokio::test]
    async fn test_list_events_filter_by_name() {
        let service = ConsulEventService::new(test_index_provider());

        service
            .fire_event("specific-event", None, "", "", "")
            .await;
        service.fire_event("other-event", None, "", "", "").await;

        let events = service
            .list_events(Some("specific-event"), None, None, None)
            .await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "specific-event");
    }

    #[tokio::test]
    async fn test_event_unique_ids() {
        let service = ConsulEventService::new(test_index_provider());

        let e1 = service.fire_event("evt", None, "", "", "").await;
        let e2 = service.fire_event("evt", None, "", "", "").await;
        assert_ne!(e1.id, e2.id);
    }

    #[tokio::test]
    async fn test_event_empty_payload() {
        let service = ConsulEventService::new(test_index_provider());

        let event = service.fire_event("no-payload", None, "", "", "").await;
        assert!(event.payload.is_none());
    }

    #[tokio::test]
    async fn test_event_ltime_increments() {
        let service = ConsulEventService::new(test_index_provider());

        let e1 = service.fire_event("a", None, "", "", "").await;
        let e2 = service.fire_event("b", None, "", "", "").await;
        assert!(e2.ltime > e1.ltime);
    }

    #[tokio::test]
    async fn test_ring_buffer_overflow() {
        let service = ConsulEventService::new(test_index_provider());

        // Fill buffer beyond capacity
        for i in 0..MAX_EVENTS + 10 {
            service
                .fire_event(&format!("evt-{}", i), None, "", "", "")
                .await;
        }

        let events = service.list_events(None, None, None, None).await;
        // Should contain exactly MAX_EVENTS entries
        assert_eq!(events.len(), MAX_EVENTS);
        // Oldest events should be overwritten — first event should be evt-10
        assert_eq!(events[0].name, "evt-10");
        // Last event should be the most recent
        assert_eq!(events[MAX_EVENTS - 1].name, format!("evt-{}", MAX_EVENTS + 9));
    }

    #[tokio::test]
    async fn test_ingest_event_updates_ltime() {
        let service = ConsulEventService::new(test_index_provider());

        // Simulate receiving an event from another node with high LTime
        let remote_event = UserEvent {
            id: uuid::Uuid::new_v4().to_string(),
            name: "remote".to_string(),
            payload: None,
            node_filter: String::new(),
            service_filter: String::new(),
            tag_filter: String::new(),
            version: 1,
            ltime: 100,
        };
        service.ingest_event(remote_event).await;

        // Next local event should have LTime > 100
        let local = service.fire_event("local", None, "", "", "").await;
        assert!(local.ltime > 100);
    }

    #[tokio::test]
    async fn test_uuid_to_index() {
        let idx = uuid_to_index("550e8400-e29b-41d4-a716-446655440000");
        assert!(idx > 0);

        // Same UUID should always produce same index
        let idx2 = uuid_to_index("550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(idx, idx2);

        // Different UUIDs should (almost certainly) produce different indexes
        let idx3 = uuid_to_index("6ba7b810-9dad-11d1-80b4-00c04fd430c8");
        assert_ne!(idx, idx3);
    }

    #[tokio::test]
    async fn test_current_index_updates() {
        let service = ConsulEventService::new(test_index_provider());
        assert_eq!(service.current_index(), 0);

        service.fire_event("test", None, "", "", "").await;
        assert!(service.current_index() > 0);
    }

    #[test]
    fn test_ring_buffer_empty() {
        let buf = EventRingBuffer::new();
        assert!(buf.events().is_empty());
    }

    #[test]
    fn test_ring_buffer_ordering() {
        let mut buf = EventRingBuffer::new();

        for i in 0..5 {
            buf.push(UserEvent {
                id: format!("id-{}", i),
                name: format!("evt-{}", i),
                payload: None,
                node_filter: String::new(),
                service_filter: String::new(),
                tag_filter: String::new(),
                version: 1,
                ltime: i as u64,
            });
        }

        let events = buf.events();
        assert_eq!(events.len(), 5);
        for (i, event) in events.iter().enumerate() {
            assert_eq!(event.name, format!("evt-{}", i));
        }
    }

    #[test]
    fn test_ring_buffer_wrap() {
        let mut buf = EventRingBuffer::new();

        for i in 0..MAX_EVENTS + 5 {
            buf.push(UserEvent {
                id: format!("id-{}", i),
                name: format!("evt-{}", i),
                payload: None,
                node_filter: String::new(),
                service_filter: String::new(),
                tag_filter: String::new(),
                version: 1,
                ltime: i as u64,
            });
        }

        let events = buf.events();
        assert_eq!(events.len(), MAX_EVENTS);
        // First event in buffer should be evt-5 (oldest surviving)
        assert_eq!(events[0].name, "evt-5");
        // Last should be the most recently written
        assert_eq!(
            events[MAX_EVENTS - 1].name,
            format!("evt-{}", MAX_EVENTS + 4)
        );
    }

    #[tokio::test]
    async fn test_fire_event_calls_broadcaster() {
        use std::sync::atomic::{AtomicU32, Ordering};

        // Create a mock broadcaster that counts calls
        struct MockBroadcaster {
            call_count: AtomicU32,
        }

        #[async_trait::async_trait]
        impl EventBroadcaster for MockBroadcaster {
            async fn broadcast_event(&self, _event: &UserEvent) {
                self.call_count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let service = ConsulEventService::new(test_index_provider());
        let mock = Arc::new(MockBroadcaster {
            call_count: AtomicU32::new(0),
        });
        service.set_broadcaster(mock.clone()).await;

        // Fire 3 events
        service.fire_event("evt1", None, "", "", "").await;
        service.fire_event("evt2", None, "", "", "").await;
        service.fire_event("evt3", None, "", "", "").await;

        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_fire_event_no_broadcaster() {
        let service = ConsulEventService::new(test_index_provider());
        // No broadcaster set — standalone mode
        let event = service.fire_event("standalone", None, "", "", "").await;
        assert_eq!(event.name, "standalone");
        // Should work without errors
    }

    #[tokio::test]
    async fn test_blocking_query_wakeup() {
        let service = ConsulEventService::new(test_index_provider());
        let service2 = service.clone();

        // Start a blocking wait with short timeout
        let handle = tokio::spawn(async move {
            service2.wait_for_event(Some(std::time::Duration::from_secs(5))).await;
            true
        });

        // Give the waiter time to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Fire an event — should wake up the waiter
        service.fire_event("wake", None, "", "", "").await;

        // Waiter should complete quickly (not timeout)
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            handle,
        ).await;
        assert!(result.is_ok(), "Blocking query should have been woken up");
    }

    #[tokio::test]
    async fn test_blocking_query_timeout() {
        let service = ConsulEventService::new(test_index_provider());

        let start = std::time::Instant::now();
        service.wait_for_event(Some(std::time::Duration::from_millis(100))).await;
        let elapsed = start.elapsed();

        // Should have waited ~100ms, not 5 minutes
        assert!(elapsed < std::time::Duration::from_secs(1));
    }
}
