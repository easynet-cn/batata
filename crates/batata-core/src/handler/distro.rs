//! Distro protocol gRPC handlers
//!
//! Implements handlers for Distro cluster synchronization requests.
//! These handlers are used for AP mode (ephemeral instance) data synchronization.

use std::sync::Arc;

use crate::{
    model::Connection,
    service::distro::{
        DistroData, DistroDataType, DistroProtocol, custom_type_name, distro_content_to_string,
    },
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
                let type_name = custom_type_name(request.distro_data.custom_type_name.clone());
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
                        let type_name = custom_type_name(item.custom_type_name.clone());
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

/// Maximum number of in-flight `is_in_sync_with` probes for one verify
/// request. Bounds the fan-out so a peer with many keys cannot start an
/// unbounded number of concurrent reads.
const MAX_CONCURRENT_VERIFY_PROBES: usize = 16;

/// Probe every key in `verify_data` with bounded concurrency and return the
/// keys the local replica cannot prove it holds at `source`'s version.
///
/// The probes are independent reads, so a sequential loop let one slow key
/// serialise the whole verify round. `buffer_unordered` keeps up to
/// `MAX_CONCURRENT_VERIFY_PROBES` in flight. `verify_data` is a `HashMap`, so
/// its iteration order is not significant; only the SET of returned keys
/// matters, which is unchanged from the sequential version.
async fn keys_needing_sync(
    protocol: &Arc<DistroProtocol>,
    data_type: DistroDataType,
    source: String,
    verify_data: &std::collections::HashMap<String, i64>,
) -> Vec<String> {
    use futures::StreamExt;
    use futures::future::BoxFuture;

    let dt = Arc::new(data_type);
    let src = Arc::new(source);
    let protocol = Arc::clone(protocol);
    // `map` returning an async block makes the future non-'static enough for
    // `buffer_unordered`; box each probe so the stream yields `BoxFuture<'static>`.
    let probes: Vec<BoxFuture<'static, (String, bool)>> = verify_data
        .iter()
        .map(move |(key, version)| {
            let protocol = Arc::clone(&protocol);
            let dt = Arc::clone(&dt);
            let src = Arc::clone(&src);
            let key = key.clone();
            let version = *version;
            Box::pin(async move {
                let in_sync = protocol
                    .is_in_sync_with(dt.as_ref(), &key, src.as_str(), version)
                    .await;
                (key, in_sync)
            }) as BoxFuture<'static, (String, bool)>
        })
        .collect();
    let mut probes = futures::stream::iter(probes).buffer_unordered(MAX_CONCURRENT_VERIFY_PROBES);

    let mut keys_need_sync = Vec::new();
    while let Some((key, in_sync)) = probes.next().await {
        if !in_sync {
            keys_need_sync.push(key);
        }
    }
    keys_need_sync
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
                let type_name = custom_type_name(request.custom_type_name.clone());
                DistroDataType::Custom(type_name)
            }
        };

        let source = request.source.clone();
        if source.is_empty() {
            // Peer predates the `source` field (or is misconfigured). Without
            // it, per-source versions cannot be compared, so report every key
            // as stale — one redundant round of syncs, then it converges once
            // the peer sends its address.
            warn!("Distro verify request without source address; reporting all keys stale");
        }

        // Probe all keys with bounded concurrency instead of awaiting each
        // one sequentially.
        let keys_need_sync = keys_needing_sync(
            &self.distro_protocol,
            data_type,
            source,
            &request.verify_data,
        )
        .await;

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
                let type_name = custom_type_name(request.custom_type_name.clone());
                DistroDataType::Custom(type_name)
            }
        };

        // Get snapshot
        let snapshot = self.distro_protocol.get_snapshot(&data_type).await;

        // Convert to API format
        let api_snapshot: Vec<batata_server_api::distro::DistroDataItem> = snapshot
            .into_iter()
            .map(
                |d| -> Result<batata_server_api::distro::DistroDataItem, String> {
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
                    Ok(batata_server_api::distro::DistroDataItem {
                        data_type: api_data_type,
                        custom_type_name: custom_name,
                        content: distro_content_to_string(d.content, &d.key)?,
                        key: d.key,
                        version: d.version,
                        source: d.source,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()
            .map_err(Status::internal)?;

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
    use super::*;
    use crate::service::cluster_client::{ClusterClientConfig, ClusterClientManager};
    use crate::service::distro::{DistroConfig, DistroDataHandler};
    use dashmap::DashMap;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_handler_types() {
        // Test that handlers have correct can_handle values
        assert_eq!("DistroDataSyncRequest", "DistroDataSyncRequest");
        assert_eq!("DistroDataVerifyRequest", "DistroDataVerifyRequest");
        assert_eq!("DistroDataSnapshotRequest", "DistroDataSnapshotRequest");
    }

    /// Tracks the peak number of probes running at the same time.
    #[derive(Default)]
    struct ProbeTracker {
        active: AtomicUsize,
        max: AtomicUsize,
    }

    impl ProbeTracker {
        fn enter(&self) {
            let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max.fetch_max(now, Ordering::SeqCst);
        }

        fn leave(&self) {
            self.active.fetch_sub(1, Ordering::SeqCst);
        }

        fn max(&self) -> usize {
            self.max.load(Ordering::SeqCst)
        }
    }

    /// In-memory handler: `process_verify_data` answers "in sync" when the
    /// local version is >= the probed version, and records concurrency.
    struct MockHandler {
        versions: HashMap<String, i64>,
        tracker: Arc<ProbeTracker>,
    }

    #[tonic::async_trait]
    impl DistroDataHandler for MockHandler {
        fn data_type(&self) -> DistroDataType {
            DistroDataType::NamingInstance
        }

        async fn get_all_keys(&self) -> Vec<String> {
            self.versions.keys().cloned().collect()
        }

        async fn get_data(&self, _key: &str) -> Option<DistroData> {
            None
        }

        async fn process_sync_data(&self, _data: DistroData) -> Result<(), String> {
            Ok(())
        }

        async fn process_verify_data(&self, data: &DistroData) -> Result<bool, String> {
            self.tracker.enter();
            // Yield so concurrent probes actually overlap when the fan-out
            // is unbounded/bounded, making `max` observable.
            tokio::task::yield_now().await;
            self.tracker.leave();
            Ok(self
                .versions
                .get(&data.key)
                .map(|local| *local >= data.version)
                .unwrap_or(false))
        }

        async fn get_snapshot(&self) -> Vec<DistroData> {
            vec![]
        }
    }

    fn protocol_with_versions(
        versions: HashMap<String, i64>,
        tracker: Arc<ProbeTracker>,
    ) -> Arc<DistroProtocol> {
        let client_manager = Arc::new(ClusterClientManager::new(
            "10.0.0.1:8848".to_string(),
            ClusterClientConfig::default(),
        ));
        let members = Arc::new(DashMap::new());
        let proto = DistroProtocol::new(
            "10.0.0.1:8848".to_string(),
            DistroConfig::default(),
            client_manager,
            members,
        );
        proto.register_handler(Arc::new(MockHandler { versions, tracker }));
        Arc::new(proto)
    }

    /// The bounded-concurrency path must report exactly the stale keys
    /// (in-sync keys excluded, unknown/stale keys included), preserving the
    /// sequential implementation's response semantics.
    #[tokio::test]
    async fn test_keys_needing_sync_reports_only_stale_keys() {
        let mut versions = HashMap::new();
        versions.insert("fresh".to_string(), 10i64);
        versions.insert("stale".to_string(), 2i64);
        let proto = protocol_with_versions(versions, Arc::new(ProbeTracker::default()));

        let mut verify_data = HashMap::new();
        verify_data.insert("fresh".to_string(), 10i64); // local == peer -> in sync
        verify_data.insert("stale".to_string(), 5i64); // local(2) < peer(5) -> stale
        verify_data.insert("missing".to_string(), 1i64); // no local copy -> stale

        let mut result = keys_needing_sync(
            &proto,
            DistroDataType::NamingInstance,
            "10.0.0.2:8848".to_string(),
            &verify_data,
        )
        .await;
        result.sort();
        assert_eq!(
            result,
            vec!["missing".to_string(), "stale".to_string()],
            "only keys the replica cannot prove current must be reported"
        );
    }

    /// The verify fan-out must stay capped: with far more keys than the
    /// limit, the peak in-flight probes must never exceed the bound.
    #[tokio::test]
    async fn test_keys_needing_sync_caps_concurrency() {
        let tracker = Arc::new(ProbeTracker::default());
        let mut verify_data = HashMap::new();
        for i in 0..64 {
            verify_data.insert(format!("k{}", i), 1i64);
        }
        let proto = protocol_with_versions(HashMap::new(), Arc::clone(&tracker));

        let result = keys_needing_sync(
            &proto,
            DistroDataType::NamingInstance,
            "src".to_string(),
            &verify_data,
        )
        .await;

        // Every key is stale (no local copy), so all are reported.
        assert_eq!(result.len(), 64);
        // The mock yields inside each probe, so `buffer_unordered` produces a
        // real overlap. A sequential loop keeps the peak at 1 and fails here.
        assert!(
            tracker.max() > 1,
            "verify probes must overlap; a sequential loop would peak at 1"
        );
        assert!(
            tracker.max() <= MAX_CONCURRENT_VERIFY_PROBES,
            "verify probes must be bounded by {}, saw {}",
            MAX_CONCURRENT_VERIFY_PROBES,
            tracker.max()
        );
    }
}
