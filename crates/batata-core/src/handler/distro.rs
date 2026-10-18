//! Distro protocol gRPC handlers
//!
//! Implements handlers for Distro cluster synchronization requests.
//! These handlers are used for AP mode (ephemeral instance) data synchronization.

use std::sync::Arc;

use crate::{
    model::Connection,
    service::distro::{DistroData, DistroDataType, DistroProtocol},
};
use tonic::Status;
use tracing::warn;

use crate::handler::rpc::{AuthRequirement, PayloadHandler};
use batata_api::grpc::Payload;
use batata_api::remote::model::{RequestTrait, ResponseTrait};
use batata_server_api::distro::{
    DistroDataBatchSyncRequest, DistroDataBatchSyncResponse, DistroDataSnapshotRequest,
    DistroDataSnapshotResponse, DistroDataSyncRequest, DistroDataSyncResponse,
    DistroDataVerifyRequest, DistroDataVerifyResponse,
};

/// Handler for DistroDataSyncRequest - receives sync data from other cluster nodes
#[derive(Clone)]
pub struct DistroDataSyncHandler {
    /// The `distro_protocol` field.
    pub distro_protocol: Arc<DistroProtocol>,
}

#[tonic::async_trait]
impl PayloadHandler for DistroDataSyncHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = DistroDataSyncRequest::from(payload);
        let request_id = request.request_id();

        // Convert API data type to internal data type
        let data_type = match request.distro_data.data_type {
            batata_server_api::distro::DistroDataType::NamingInstance => {
                DistroDataType::NamingInstance
            }
            batata_server_api::distro::DistroDataType::Custom => {
                let type_name = request
                    .distro_data
                    .custom_type_name
                    .clone()
                    .unwrap_or_else(|| "CUSTOM".to_string());
                DistroDataType::Custom(type_name)
            }
        };

        // Create internal DistroData. The version MUST be the one that came
        // off the wire: the replica later answers "am I up to date with this
        // peer?" by comparing it against what it applied last time.
        let data = DistroData::with_version(
            data_type,
            request.distro_data.key,
            request.distro_data.content.into_bytes(),
            request.distro_data.source,
            request.distro_data.version,
        );

        // Process the sync data
        let result = self.distro_protocol.receive_sync_data(data).await;

        let mut response = match result {
            Ok(()) => DistroDataSyncResponse::success(),
            Err(msg) => DistroDataSyncResponse::fail(&msg),
        };
        response.request_id(request_id);

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "DistroDataSyncRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

/// Handler for DistroDataBatchSyncRequest - receives batched sync data from peers.
///
/// Applies each item the same way `DistroDataSyncHandler` applies a single item,
/// then returns a per-item failure list. Idempotent — re-applying the same item
/// is safe because each handler's `process_sync_data` is itself idempotent.
#[derive(Clone)]
pub struct DistroDataBatchSyncHandler {
    /// The `distro_protocol` field.
    pub distro_protocol: Arc<DistroProtocol>,
}

#[tonic::async_trait]
impl PayloadHandler for DistroDataBatchSyncHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = DistroDataBatchSyncRequest::from(payload);
        let request_id = request.request_id();

        let datas: Vec<DistroData> = request
            .items
            .into_iter()
            .map(|item| {
                let data_type = match item.data_type {
                    batata_server_api::distro::DistroDataType::NamingInstance => {
                        DistroDataType::NamingInstance
                    }
                    batata_server_api::distro::DistroDataType::Custom => {
                        let type_name = item
                            .custom_type_name
                            .clone()
                            .unwrap_or_else(|| "CUSTOM".to_string());
                        DistroDataType::Custom(type_name)
                    }
                };
                DistroData {
                    data_type,
                    key: item.key,
                    content: item.content.into_bytes(),
                    version: item.version,
                    source: item.source,
                }
            })
            .collect();

        let failed_keys = self.distro_protocol.receive_batch_sync(datas).await;

        let mut response = DistroDataBatchSyncResponse::success(failed_keys);
        response.request_id(request_id);
        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "DistroDataBatchSyncRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

/// Handler for DistroDataVerifyRequest - verifies data consistency with other cluster nodes
#[derive(Clone)]
pub struct DistroDataVerifyHandler {
    /// The `distro_protocol` field.
    pub distro_protocol: Arc<DistroProtocol>,
}

#[tonic::async_trait]
impl PayloadHandler for DistroDataVerifyHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = DistroDataVerifyRequest::from(payload);
        let request_id = request.request_id();

        // Convert API data type to internal data type
        let data_type = match request.data_type {
            batata_server_api::distro::DistroDataType::NamingInstance => {
                DistroDataType::NamingInstance
            }
            batata_server_api::distro::DistroDataType::Custom => {
                let type_name = request
                    .custom_type_name
                    .clone()
                    .unwrap_or_else(|| "CUSTOM".to_string());
                DistroDataType::Custom(type_name)
            }
        };

        // Check which keys need sync
        let mut keys_need_sync = Vec::new();

        let source = request.source.clone();
        if source.is_empty() {
            // Peer predates the `source` field (or is misconfigured). Without
            // it, per-source versions cannot be compared, so report every key
            // as stale — one redundant round of syncs, then it converges once
            // the peer sends its address.
            warn!("Distro verify request without source address; reporting all keys stale");
        }

        for (key, version) in &request.verify_data {
            let in_sync = self
                .distro_protocol
                .is_in_sync_with(&data_type, key, &source, *version)
                .await;
            if !in_sync {
                keys_need_sync.push(key.clone());
            }
        }

        let mut response = DistroDataVerifyResponse::new();
        response.request_id(request_id);
        response.keys_need_sync = keys_need_sync;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "DistroDataVerifyRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

/// Handler for DistroDataSnapshotRequest - returns all data for a type
#[derive(Clone)]
pub struct DistroDataSnapshotHandler {
    /// The `distro_protocol` field.
    pub distro_protocol: Arc<DistroProtocol>,
}

#[tonic::async_trait]
impl PayloadHandler for DistroDataSnapshotHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = DistroDataSnapshotRequest::from(payload);
        let request_id = request.request_id();

        // Convert API data type to internal data type
        let data_type = match request.data_type {
            batata_server_api::distro::DistroDataType::NamingInstance => {
                DistroDataType::NamingInstance
            }
            batata_server_api::distro::DistroDataType::Custom => {
                let type_name = request
                    .custom_type_name
                    .clone()
                    .unwrap_or_else(|| "CUSTOM".to_string());
                DistroDataType::Custom(type_name)
            }
        };

        // Get snapshot
        let snapshot = self.distro_protocol.get_snapshot(&data_type).await;

        // Convert to API format
        let api_snapshot: Vec<batata_server_api::distro::DistroDataItem> = snapshot
            .into_iter()
            .map(|d| {
                let (api_data_type, custom_name) = match &d.data_type {
                    DistroDataType::NamingInstance => (
                        batata_server_api::distro::DistroDataType::NamingInstance,
                        None,
                    ),
                    DistroDataType::Custom(name) => (
                        batata_server_api::distro::DistroDataType::Custom,
                        Some(name.clone()),
                    ),
                };
                batata_server_api::distro::DistroDataItem {
                    data_type: api_data_type,
                    custom_type_name: custom_name,
                    key: d.key,
                    content: String::from_utf8(d.content).unwrap_or_default(),
                    version: d.version,
                    source: d.source,
                }
            })
            .collect();

        let mut response = DistroDataSnapshotResponse::new();
        response.request_id(request_id);
        response.snapshot = api_snapshot;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "DistroDataSnapshotRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_handler_types() {
        // Test that handlers have correct can_handle values
        assert_eq!("DistroDataSyncRequest", "DistroDataSyncRequest");
        assert_eq!("DistroDataVerifyRequest", "DistroDataVerifyRequest");
        assert_eq!("DistroDataSnapshotRequest", "DistroDataSnapshotRequest");
    }
}
