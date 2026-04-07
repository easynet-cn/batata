//! Consul event broadcast handler and broadcaster implementation.
//!
//! - `ConsulEventBroadcastHandler`: gRPC handler that receives event broadcasts
//!   from peer nodes and ingests them into the local ring buffer.
//! - `ConsulEventBroadcasterImpl`: concrete `EventBroadcaster` implementation
//!   that uses `ClusterClientManager` to broadcast events to all cluster peers.

use std::sync::Arc;

use tonic::Status;
use tracing::{debug, warn};

use batata_api::remote::model::{
    ConsulEventBroadcastRequest, ConsulEventBroadcastResponse, RequestTrait, ResponseTrait,
};
use batata_core::handler::rpc::{AuthRequirement, PayloadHandler};
use batata_core::model::Connection;
use batata_core::service::cluster_client::ClusterClientManager;
use batata_plugin_consul::ConsulEventService;
use batata_plugin_consul::event::EventBroadcaster;
use batata_plugin_consul::model::UserEvent;

use batata_api::Member;
use batata_common::ClusterManager;

// ============================================================================
// gRPC Handler: receives event broadcasts from peers
// ============================================================================

/// Handles `ConsulEventBroadcastRequest` from peer nodes.
///
/// When a node fires an event, it broadcasts to all peers. Each peer's handler
/// ingests the event into its local ring buffer, matching Consul's gossip behavior.
pub struct ConsulEventBroadcastHandler {
    pub event_service: ConsulEventService,
}

#[tonic::async_trait]
impl PayloadHandler for ConsulEventBroadcastHandler {
    async fn handle(
        &self,
        connection: &Connection,
        payload: &batata_core::api::grpc::Payload,
    ) -> Result<batata_core::api::grpc::Payload, Status> {
        let request = ConsulEventBroadcastRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            event_name = %request.event_name,
            event_id = %request.event_id,
            ltime = request.ltime,
            from = %connection.meta_info.remote_ip,
            "Received event broadcast from peer"
        );

        // Reconstruct UserEvent from broadcast request
        let event = UserEvent {
            id: request.event_id,
            name: request.event_name,
            payload: request.payload,
            node_filter: request.node_filter,
            service_filter: request.service_filter,
            tag_filter: request.tag_filter,
            version: 1,
            ltime: request.ltime,
        };

        // Ingest into local ring buffer (updates LTime, notifies blocking queries)
        self.event_service.ingest_event(event).await;

        let mut response = ConsulEventBroadcastResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConsulEventBroadcastRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

// ============================================================================
// EventBroadcaster: sends events to all cluster peers
// ============================================================================

/// Concrete implementation of `EventBroadcaster` using `ClusterClientManager`.
///
/// Broadcasts events to all healthy cluster members via gRPC (fire-and-forget).
/// Errors on individual nodes are logged but don't fail the operation.
pub struct ConsulEventBroadcasterImpl {
    client_manager: Arc<ClusterClientManager>,
    cluster_manager: Arc<dyn ClusterManager>,
}

impl ConsulEventBroadcasterImpl {
    pub fn new(
        client_manager: Arc<ClusterClientManager>,
        cluster_manager: Arc<dyn ClusterManager>,
    ) -> Self {
        Self {
            client_manager,
            cluster_manager,
        }
    }
}

#[async_trait::async_trait]
impl EventBroadcaster for ConsulEventBroadcasterImpl {
    async fn broadcast_event(&self, event: &UserEvent) {
        let members = self.cluster_manager.healthy_members_extended();
        if members.is_empty() {
            return;
        }

        let request = ConsulEventBroadcastRequest::new(
            event.id.clone(),
            event.name.clone(),
            event.payload.clone(),
            event.node_filter.clone(),
            event.service_filter.clone(),
            event.tag_filter.clone(),
            event.ltime,
        );

        let member_list: Vec<Member> = members
            .iter()
            .map(|m| {
                let port = m.port;
                Member::new(m.ip.clone(), port)
            })
            .collect();

        let results = self.client_manager.broadcast(&member_list, request).await;

        for (address, result) in &results {
            if let Err(e) = result {
                warn!(
                    peer = %address,
                    event_name = %event.name,
                    "Failed to broadcast event to peer: {}",
                    e
                );
            }
        }

        debug!(
            event_name = %event.name,
            peers = results.len(),
            "Event broadcast complete"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_plugin_consul::index_provider::ConsulIndexProvider;

    #[test]
    fn test_handler_can_handle() {
        let index_provider = ConsulIndexProvider::new();
        let event_service = ConsulEventService::new(index_provider);
        let handler = ConsulEventBroadcastHandler { event_service };
        assert_eq!(handler.can_handle(), "ConsulEventBroadcastRequest");
    }

    #[test]
    fn test_handler_auth_requirement() {
        let index_provider = ConsulIndexProvider::new();
        let event_service = ConsulEventService::new(index_provider);
        let handler = ConsulEventBroadcastHandler { event_service };
        assert!(matches!(
            handler.auth_requirement(),
            AuthRequirement::Internal
        ));
    }

    #[tokio::test]
    async fn test_ingest_via_event_service() {
        let index_provider = ConsulIndexProvider::new();
        let event_service = ConsulEventService::new(index_provider);

        // Simulate what the handler does — ingest an event
        let event = batata_plugin_consul::model::UserEvent {
            id: uuid::Uuid::new_v4().to_string(),
            name: "test-broadcast".to_string(),
            payload: Some("data".to_string()),
            node_filter: String::new(),
            service_filter: String::new(),
            tag_filter: String::new(),
            version: 1,
            ltime: 42,
        };

        event_service.ingest_event(event).await;

        let events = event_service
            .list_events(Some("test-broadcast"), None, None, None)
            .await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "test-broadcast");
        assert_eq!(events[0].ltime, 42);
    }
}
