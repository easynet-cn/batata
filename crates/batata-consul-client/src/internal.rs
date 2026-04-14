//! Internal endpoints.
//!
//! Maps to Consul Go SDK's `api/internal.go`. These endpoints are not part
//! of the stable public API but are used by Consul UI and some admin tools.

use serde::{Deserialize, Serialize};

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{QueryMeta, WriteOptions};

/// Request body for `AssignServiceVirtualIP`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AssignServiceManualVIPsRequest {
    #[serde(rename = "Service")]
    pub service: String,
    #[serde(rename = "ManualVIPs")]
    pub manual_vips: Vec<String>,
}

/// Response body.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AssignServiceManualVIPsResponse {
    #[serde(rename = "Found", default)]
    pub service_found: bool,
    #[serde(rename = "UnassignedFrom", default)]
    pub unassigned_from: Vec<PeeredServiceName>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeeredServiceName {
    #[serde(rename = "ServiceName")]
    pub service_name: CompoundServiceName,
    #[serde(rename = "Peer", default)]
    pub peer: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CompoundServiceName {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Namespace", default)]
    pub namespace: String,
    #[serde(rename = "Partition", default)]
    pub partition: String,
}

impl ConsulClient {
    /// Manually assign virtual IPs to a service. Returns the response plus
    /// any peered services that had their VIPs unassigned.
    ///
    /// Maps to Go SDK `Internal.AssignServiceVirtualIP`.
    pub async fn internal_assign_service_virtual_ip(
        &self,
        service: &str,
        manual_vips: &[String],
        opts: &WriteOptions,
    ) -> Result<(AssignServiceManualVIPsResponse, QueryMeta)> {
        let body = AssignServiceManualVIPsRequest {
            service: service.to_string(),
            manual_vips: manual_vips.to_vec(),
        };
        let (resp, _meta): (AssignServiceManualVIPsResponse, _) = self
            .put("/v1/internal/service-virtual-ip", Some(&body), opts, &[])
            .await?;
        Ok((resp, QueryMeta::default()))
    }
}
