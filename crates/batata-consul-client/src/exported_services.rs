//! Exported services (Cluster Peering read endpoint).
//!
//! Maps to Consul Go SDK's `api/exported_services.go`. Lists services
//! currently exported to peers/partitions via `exported-services` config
//! entries, after resolution of `*` wildcards and sameness-groups.

use serde::{Deserialize, Serialize};

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{QueryMeta, QueryOptions};

/// A single exported service after resolution.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResolvedExportedService {
    /// Service name being exported.
    #[serde(rename = "Service")]
    pub service: String,
    /// Partition of the exported service.
    #[serde(rename = "Partition", default, skip_serializing_if = "String::is_empty")]
    pub partition: String,
    /// Namespace of the exported service.
    #[serde(rename = "Namespace", default, skip_serializing_if = "String::is_empty")]
    pub namespace: String,
    /// Consumers (peers / partitions) that can see this service.
    #[serde(rename = "Consumers", default)]
    pub consumers: ResolvedConsumers,
}

/// Consumers resolved for an export.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResolvedConsumers {
    #[serde(rename = "Peers", default, skip_serializing_if = "Vec::is_empty")]
    pub peers: Vec<String>,
    #[serde(rename = "Partitions", default, skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<String>,
}

impl ConsulClient {
    /// List all currently exported services for the caller's partition.
    ///
    /// Matches Go SDK `Client.ExportedServices(q)`.
    pub async fn exported_services(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<ResolvedExportedService>, QueryMeta)> {
        self.get("/v1/exported-services", opts).await
    }
}
