//! Imported services (Cluster Peering read endpoint).
//!
//! Maps to Consul Go SDK's `api/imported_services.go`. Lists services
//! imported from remote peer clusters.

use serde::{Deserialize, Serialize};

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{QueryMeta, QueryOptions};

/// A single imported service record.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ImportedService {
    /// Local name of the imported service.
    #[serde(rename = "Service")]
    pub service: String,
    /// Partition this service appears under in the local cluster.
    #[serde(rename = "Partition", default, skip_serializing_if = "String::is_empty")]
    pub partition: String,
    /// Namespace in the local cluster.
    #[serde(rename = "Namespace", default, skip_serializing_if = "String::is_empty")]
    pub namespace: String,
    /// Peer from which this service was imported.
    #[serde(rename = "SourcePeer", default, skip_serializing_if = "String::is_empty")]
    pub source_peer: String,
    /// Partition of the source peer.
    #[serde(
        rename = "SourcePartition",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub source_partition: String,
}

/// Wire envelope returned by `/v1/imported-services`.
#[derive(Clone, Debug, Default, Deserialize)]
struct ImportedServicesResponse {
    #[serde(rename = "Partition", default)]
    _partition: String,
    #[serde(rename = "ImportedServices", default)]
    imported_services: Vec<ImportedService>,
}

impl ConsulClient {
    /// List all currently imported services for the caller's partition.
    ///
    /// Matches Go SDK `Client.ImportedServices(q)`.
    pub async fn imported_services(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<ImportedService>, QueryMeta)> {
        let (resp, meta): (ImportedServicesResponse, QueryMeta) =
            self.get("/v1/imported-services", opts).await?;
        Ok((resp.imported_services, meta))
    }
}
