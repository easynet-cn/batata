// Cluster module gRPC handler: MemberReportHandler
// Handles cluster member heartbeat reporting between nodes

use std::sync::Arc;

use tonic::Status;
use tracing::info;

use crate::model::Connection;
use batata_api::model::NodeState;

use crate::{
    api::{
        grpc::Payload,
        remote::model::{MemberReportRequest, MemberReportResponse, RequestTrait, ResponseTrait},
    },
    handler::rpc::{AuthRequirement, PayloadHandler},
    service::cluster::ServerMemberManager,
};

/// Handler for MemberReportRequest - processes cluster member heartbeat reports
#[derive(Clone)]
pub struct MemberReportHandler {
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

#[cfg(test)]
mod tests {
    use crate::api::remote::model::RequestTrait;

    #[test]
    fn test_member_report_request_type() {
        let req = super::MemberReportRequest::default();
        assert_eq!(req.request_type(), "MemberReportRequest");
    }
}
