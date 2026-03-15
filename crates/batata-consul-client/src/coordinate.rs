//! Coordinate API for network latency measurement

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{
    DatacenterCoordinate, NodeCoordinate, QueryMeta, QueryOptions, WriteMeta, WriteOptions,
};

/// Coordinate operations
impl ConsulClient {
    /// Get WAN coordinates for all datacenters
    pub async fn coordinate_datacenters(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<DatacenterCoordinate>, QueryMeta)> {
        self.get("/v1/coordinate/datacenters", opts).await
    }

    /// Get LAN coordinates for nodes in a datacenter
    pub async fn coordinate_nodes(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<NodeCoordinate>, QueryMeta)> {
        self.get("/v1/coordinate/nodes", opts).await
    }

    /// Get coordinate for a specific node
    pub async fn coordinate_node(
        &self,
        node: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<NodeCoordinate>, QueryMeta)> {
        self.get(&format!("/v1/coordinate/node/{}", node), opts)
            .await
    }

    /// Update node coordinate
    pub async fn coordinate_update(
        &self,
        coord: &NodeCoordinate,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let (_resp, meta): (serde_json::Value, WriteMeta) = self
            .put("/v1/coordinate/update", Some(coord), opts, &[])
            .await?;
        Ok(meta)
    }
}
