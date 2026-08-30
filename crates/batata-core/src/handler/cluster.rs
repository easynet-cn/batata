// Cluster module gRPC handlers
//
// Handles cluster-internal gRPC requests between nodes:
// - MemberReportHandler: cluster member heartbeat reporting
// - PluginAvailabilityHandler: plugin availability queries

use std::collections::HashMap;
use std::sync::Arc;

use tonic::Status;
use tracing::{debug, info};

use crate::model::Connection;
use batata_api::model::NodeState;

use crate::{
    api::{
        grpc::Payload,
        remote::model::{
            MemberReportRequest, MemberReportResponse, PluginAvailabilityRequest,
            PluginAvailabilityResponse, RequestTrait, ResponseTrait,
        },
    },
    handler::rpc::{AuthRequirement, PayloadHandler},
    service::cluster::ServerMemberManager,
};

/// Handler for MemberReportRequest - processes cluster member heartbeat reports
#[derive(Clone)]
pub struct MemberReportHandler {
    /// The `member_manager` field.
    pub member_manager: Arc<ServerMemberManager>,
}

#[tonic::async_trait]
impl PayloadHandler for MemberReportHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = MemberReportRequest::from(payload);
        let request_id = request.request_id();

        let Some(ref node) = request.node else {
            let response = crate::error_response!(
                MemberReportResponse,
                request_id,
                "Missing node in MemberReportRequest"
            );
            return Ok(response.build_payload());
        };

        info!(
            address = %node.address,
            state = %node.state,
            "Received member report"
        );

        // Update the reporting member's state in ServerMemberManager
        self.member_manager
            .update_member_state(&node.address, NodeState::Up)
            .await;

        // Return self member info
        let self_member = Some(self.member_manager.get_self().clone());

        let mut response = MemberReportResponse::new();
        response.response.request_id = request_id;
        response.node = self_member;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "MemberReportRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

// ---------------------------------------------------------------------------
// Plugin Availability
// ---------------------------------------------------------------------------

/// Trait for providing plugin availability information without coupling
/// `batata-core` to `batata-server-common` or `batata-server`.
///
/// Implemented in `batata-server` where `AppState` (with configuration and
/// plugin manager) is available.
///
/// Mirrors Nacos `PluginManager` / `PluginProviderService` which knows about
/// all registered plugins and their enabled state.
pub trait PluginAvailabilityProvider: Send + Sync {
    /// Return a map of `plugin_id` (format `type:name`) -> enabled for all
    /// known plugins on this node.
    fn plugin_availability(&self) -> HashMap<String, bool>;
}

/// Handler for PluginAvailabilityRequest (cluster-internal).
///
/// Mirrors Nacos `PluginAvailabilityRequestHandler`.
/// Returns plugin availability info for the local node.
///
/// # Logic
/// - `query_all == false` and `plugin_id` is empty -> error response
/// - `query_all == true` -> return full `plugin_availability_map`
/// - single plugin -> return `plugin_id` and `available` for that plugin
#[derive(Clone)]
pub struct PluginAvailabilityHandler {
    /// The `provider` field.
    pub provider: Arc<dyn PluginAvailabilityProvider>,
}

#[tonic::async_trait]
impl PayloadHandler for PluginAvailabilityHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = PluginAvailabilityRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            plugin_id = %request.plugin_id,
            query_all = request.query_all,
            "Received plugin availability query"
        );

        // Validation: when not querying all, plugin_id must be present
        if !request.query_all && request.plugin_id.is_empty() {
            let response = crate::error_response!(
                PluginAvailabilityResponse,
                request_id,
                "pluginId is required when queryAll is false"
            );
            return Ok(response.build_payload());
        }

        if request.query_all {
            // Return availability for all plugins
            let map = self.provider.plugin_availability();

            let mut response = PluginAvailabilityResponse::new();
            response.response.request_id = request_id;
            response.plugin_availability_map = Some(map);

            Ok(response.build_payload())
        } else {
            // Single plugin query
            let map = self.provider.plugin_availability();
            let available = map.get(&request.plugin_id).copied().unwrap_or(false);

            let mut response = PluginAvailabilityResponse::new();
            response.response.request_id = request_id;
            response.plugin_id = request.plugin_id.clone();
            response.available = available;

            Ok(response.build_payload())
        }
    }

    fn can_handle(&self) -> &'static str {
        "PluginAvailabilityRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

#[cfg(test)]
mod tests {
    use crate::api::remote::model::RequestTrait;

    #[test]
    fn test_member_report_request_type() {
        let req = super::MemberReportRequest::default();
        assert_eq!(req.request_type(), "MemberReportRequest");
    }

    #[test]
    fn test_plugin_availability_request_type() {
        let req = super::PluginAvailabilityRequest::default();
        assert_eq!(req.request_type(), "PluginAvailabilityRequest");
    }
}
