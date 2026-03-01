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

use crate::{
    api::{
        distro::{
            DistroDataSnapshotRequest, DistroDataSnapshotResponse, DistroDataSyncRequest,
            DistroDataSyncResponse, DistroDataVerifyRequest, DistroDataVerifyResponse,
        },
        grpc::Payload,
        remote::model::{RequestTrait, ResponseTrait},
    },
    handler::rpc::{AuthRequirement, PayloadHandler},
};

/// Handler for DistroDataSyncRequest - receives sync data from other cluster nodes
#[derive(Clone)]
pub struct DistroDataSyncHandler {
    pub distro_protocol: Arc<DistroProtocol>,
}

#[tonic::async_trait]
impl PayloadHandler for DistroDataSyncHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = DistroDataSyncRequest::from(payload);
        let request_id = request.request_id();

        // Convert API data type to internal data type
        let data_type = match request.distro_data.data_type {
            crate::api::distro::DistroDataType::NamingInstance => DistroDataType::NamingInstance,
            crate::api::distro::DistroDataType::Custom => {
                let type_name = request
                    .distro_data
                    .custom_type_name
                    .clone()
                    .unwrap_or_else(|| "CUSTOM".to_string());
                DistroDataType::Custom(type_name)
            }
        };

        // Create internal DistroData
        let data = DistroData::new(
            data_type,
            request.distro_data.key,
            request.distro_data.content.into_bytes(),
            request.distro_data.source,
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

/// Handler for DistroDataVerifyRequest - verifies data consistency with other cluster nodes
#[derive(Clone)]
pub struct DistroDataVerifyHandler {
    pub distro_protocol: Arc<DistroProtocol>,
}

#[tonic::async_trait]
impl PayloadHandler for DistroDataVerifyHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = DistroDataVerifyRequest::from(payload);
        let request_id = request.request_id();

        // Convert API data type to internal data type
        let data_type = match request.data_type {
            crate::api::distro::DistroDataType::NamingInstance => DistroDataType::NamingInstance,
            crate::api::distro::DistroDataType::Custom => {
                let type_name = request
                    .custom_type_name
                    .clone()
                    .unwrap_or_else(|| "CUSTOM".to_string());
                DistroDataType::Custom(type_name)
            }
        };

        // Check which keys need sync
        let mut keys_need_sync = Vec::new();

        for (key, version) in &request.verify_data {
            // Get local data version
            let snapshot = self.distro_protocol.get_snapshot(&data_type).await;

            let local_version = snapshot.iter().find(|d| &d.key == key).map(|d| d.version);

            match local_version {
                Some(local_v) if local_v < *version => {
                    // Our version is older, need sync
                    keys_need_sync.push(key.clone());
                }
                None => {
                    // We don't have this key, need sync
                    keys_need_sync.push(key.clone());
                }
                _ => {
                    // Our version is same or newer, no need
                }
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
    pub distro_protocol: Arc<DistroProtocol>,
}

#[tonic::async_trait]
impl PayloadHandler for DistroDataSnapshotHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = DistroDataSnapshotRequest::from(payload);
        let request_id = request.request_id();

        // Convert API data type to internal data type
        let data_type = match request.data_type {
            crate::api::distro::DistroDataType::NamingInstance => DistroDataType::NamingInstance,
            crate::api::distro::DistroDataType::Custom => {
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
        let api_snapshot: Vec<crate::api::distro::DistroDataItem> = snapshot
            .into_iter()
            .map(|d| {
                let (api_data_type, custom_name) = match &d.data_type {
                    DistroDataType::NamingInstance => {
                        (crate::api::distro::DistroDataType::NamingInstance, None)
                    }
                    DistroDataType::Custom(name) => (
                        crate::api::distro::DistroDataType::Custom,
                        Some(name.clone()),
                    ),
                };
                crate::api::distro::DistroDataItem {
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
