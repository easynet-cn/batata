//! Connection disconnect listener for naming.
//!
//! Equivalent to Nacos `ClientConnectionEventListener.clientDisconnected()` —
//! when a client's gRPC connection goes away, every ephemeral instance that
//! client published must be deregistered and all affected subscribers must
//! be notified. Without this listener, stale ephemeral instances linger
//! until the next Distro verify cycle (cluster mode) or forever (standalone).
//!
//! This listener is wired into the batata-core `ConnectionManager` at startup
//! so it fires on every unregister path: explicit bi-stream close, stale
//! ejection by the health checker, push-failure circuit breaker, etc.

use std::sync::Arc;

use batata_api::naming::model::NotifySubscriberRequest;
use batata_api::remote::model::RequestTrait;
use batata_core::model::ConnectionMeta;
use batata_core::service::distro::{DistroDataType, DistroProtocol};
use batata_core::service::remote::ConnectionEventListener;
use tracing::{debug, info, warn};

use crate::NamingService;
use crate::service::parse_service_key;

/// Trait for pushing payloads to subscriber connections.
///
/// Abstracted so that unit tests can verify notification behavior without
/// spinning up a real gRPC channel.
#[async_trait::async_trait]
pub trait SubscriberPusher: Send + Sync {
    /// Push a payload to all given connection ids.
    async fn push_to_many(
        &self,
        connection_ids: &[String],
        payload: batata_api::grpc::Payload,
    ) -> usize;
}

/// Adapter around `batata_core::service::remote::ConnectionManager`.
pub struct ConnectionManagerPusher {
    cm: Arc<batata_core::service::remote::ConnectionManager>,
}

impl ConnectionManagerPusher {
    pub fn new(cm: Arc<batata_core::service::remote::ConnectionManager>) -> Self {
        Self { cm }
    }
}

#[async_trait::async_trait]
impl SubscriberPusher for ConnectionManagerPusher {
    async fn push_to_many(
        &self,
        connection_ids: &[String],
        payload: batata_api::grpc::Payload,
    ) -> usize {
        self.cm.push_message_to_many(connection_ids, payload).await
    }
}

/// Naming-side listener that cleans up a disconnected client's publications.
pub struct NamingDisconnectListener {
    naming: Arc<NamingService>,
    pusher: Arc<dyn SubscriberPusher>,
    /// Optional Distro protocol for pushing the removal to cluster peers.
    /// `None` in standalone mode.
    distro: Option<Arc<DistroProtocol>>,
}

impl NamingDisconnectListener {
    pub fn new(
        naming: Arc<NamingService>,
        pusher: Arc<dyn SubscriberPusher>,
        distro: Option<Arc<DistroProtocol>>,
    ) -> Self {
        Self {
            naming,
            pusher,
            distro,
        }
    }

    /// Core cleanup routine, exposed for tests.
    pub async fn handle_disconnect(&self, connection_id: &str) {
        let affected = self.naming.deregister_all_by_connection(connection_id);

        // Always remove subscriber state — even if there were no publications,
        // this client may have been a subscriber.
        self.naming.remove_subscriber(connection_id);

        if affected.is_empty() {
            debug!(
                connection_id,
                "Disconnect cleanup: no affected services for this connection"
            );
            return;
        }

        info!(
            connection_id,
            affected_count = affected.len(),
            "Disconnect cleanup: deregistering ephemeral instances for disconnected client"
        );

        // Notify local subscribers of each affected service.
        for service_key in &affected {
            if let Some((namespace, group_name, service_name)) = parse_service_key(service_key) {
                let subscribers =
                    self.naming
                        .get_subscribers(&namespace, &group_name, &service_name);
                if subscribers.is_empty() {
                    continue;
                }
                let service_info =
                    self.naming
                        .get_service(&namespace, &group_name, &service_name, "", false);
                let notification = NotifySubscriberRequest::for_service(
                    &namespace,
                    &group_name,
                    &service_name,
                    service_info,
                );
                let payload = notification.build_server_push_payload();
                let sent = self.pusher.push_to_many(&subscribers, payload).await;
                debug!(
                    service_key = %service_key,
                    sent,
                    total = subscribers.len(),
                    "Pushed disconnect-driven service change to subscribers"
                );
            } else {
                warn!(
                    service_key,
                    "Invalid service key format during disconnect cleanup"
                );
            }
        }

        // Push removals to cluster peers via Distro (cluster mode only).
        if let Some(ref distro) = self.distro {
            for key in &affected {
                distro.sync_data(DistroDataType::NamingInstance, key).await;
            }
        }
    }
}

#[async_trait::async_trait]
impl ConnectionEventListener for NamingDisconnectListener {
    async fn on_connected(&self, _connection_id: &str, _meta: &ConnectionMeta) {
        // No-op: naming does not track connect events here.
    }

    async fn on_disconnected(&self, connection_id: &str, _meta: &ConnectionMeta) {
        self.handle_disconnect(connection_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    use crate::model::Instance;
    use crate::service::{NamingService, build_service_key};

    /// Test pusher that records all push calls.
    struct RecordingPusher {
        sent: Mutex<Vec<(Vec<String>, usize)>>,
    }

    impl RecordingPusher {
        fn new() -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
            }
        }

        fn total_events(&self) -> usize {
            self.sent.lock().unwrap().len()
        }
    }

    #[async_trait::async_trait]
    impl SubscriberPusher for RecordingPusher {
        async fn push_to_many(
            &self,
            connection_ids: &[String],
            _payload: batata_api::grpc::Payload,
        ) -> usize {
            let n = connection_ids.len();
            self.sent.lock().unwrap().push((connection_ids.to_vec(), n));
            n
        }
    }

    fn make_instance(ip: &str, port: i32) -> Instance {
        let mut inst = Instance::default();
        inst.ip = ip.to_string();
        inst.port = port;
        inst.cluster_name = "DEFAULT".to_string();
        inst.ephemeral = true;
        inst.healthy = true;
        inst
    }

    #[tokio::test]
    async fn disconnect_removes_connection_instances_and_notifies_subscribers() {
        let naming = Arc::new(NamingService::new());
        let ns = "public";
        let group = "DEFAULT_GROUP";
        let svc_a = "svc-A";
        let svc_b = "svc-B";

        // Register two ephemeral instances from the same connection.
        let inst_a = make_instance("10.0.0.1", 8080);
        let inst_b = make_instance("10.0.0.1", 9090);
        assert!(naming.register_instance(ns, group, svc_a, inst_a.clone()));
        assert!(naming.register_instance(ns, group, svc_b, inst_b.clone()));

        let conn_id = "conn-publisher";
        let key_a = build_service_key(ns, group, svc_a);
        let key_b = build_service_key(ns, group, svc_b);
        let inst_key_a =
            crate::service::build_instance_key_parts(&inst_a.ip, inst_a.port, &inst_a.cluster_name);
        let inst_key_b =
            crate::service::build_instance_key_parts(&inst_b.ip, inst_b.port, &inst_b.cluster_name);
        naming.add_connection_instance(conn_id, &key_a, &inst_key_a);
        naming.add_connection_instance(conn_id, &key_b, &inst_key_b);

        // Also register a subscriber on svc-A from another connection.
        let subscriber_conn = "conn-subscriber";
        naming.subscribe(subscriber_conn, ns, group, svc_a);

        // Sanity: both instances are visible before disconnect.
        assert_eq!(
            naming.get_instances(ns, group, svc_a, "", false).len(),
            1,
            "svc-A should have 1 instance before disconnect"
        );
        assert_eq!(
            naming.get_instances(ns, group, svc_b, "", false).len(),
            1,
            "svc-B should have 1 instance before disconnect"
        );

        let pusher = Arc::new(RecordingPusher::new());
        let listener = NamingDisconnectListener::new(naming.clone(), pusher.clone(), None);

        // Fire disconnect.
        listener.handle_disconnect(conn_id).await;

        // Both services must have zero instances after cleanup.
        assert_eq!(
            naming.get_instances(ns, group, svc_a, "", false).len(),
            0,
            "svc-A must be empty after disconnect cleanup"
        );
        assert_eq!(
            naming.get_instances(ns, group, svc_b, "", false).len(),
            0,
            "svc-B must be empty after disconnect cleanup"
        );

        // The subscriber on svc-A must have received exactly one notify event.
        let events = pusher.sent.lock().unwrap().clone();
        let svc_a_events: Vec<_> = events
            .iter()
            .filter(|(conns, _)| conns.contains(&subscriber_conn.to_string()))
            .collect();
        assert_eq!(
            svc_a_events.len(),
            1,
            "subscriber for svc-A should receive exactly one notification"
        );
        assert_eq!(svc_a_events[0].1, 1, "push target count should be 1");

        // No stray extra events beyond svc-A (svc-B has no subscribers so push is skipped).
        assert_eq!(
            pusher.total_events(),
            1,
            "should only push to services that have subscribers"
        );
    }

    #[tokio::test]
    async fn disconnect_with_no_publications_is_noop() {
        let naming = Arc::new(NamingService::new());
        let pusher = Arc::new(RecordingPusher::new());
        let listener = NamingDisconnectListener::new(naming.clone(), pusher.clone(), None);

        listener.handle_disconnect("ghost-conn").await;

        assert_eq!(
            pusher.total_events(),
            0,
            "disconnect with no publications must not push any notifications"
        );
    }
}
