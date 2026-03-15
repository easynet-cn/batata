//! Peering API for cross-datacenter federation

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::*;

impl ConsulClient {
    /// Generate a peering token
    pub async fn peering_generate_token(
        &self,
        req: &PeeringGenerateTokenRequest,
        opts: &WriteOptions,
    ) -> Result<(PeeringToken, WriteMeta)> {
        self.post("/v1/peering/token", Some(req), opts, &[]).await
    }

    /// Establish a peering connection
    pub async fn peering_establish(
        &self,
        req: &PeeringEstablishRequest,
        opts: &WriteOptions,
    ) -> Result<(Peering, WriteMeta)> {
        self.post("/v1/peering/establish", Some(req), opts, &[])
            .await
    }

    /// Get a peering by name
    pub async fn peering_read(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(Peering, QueryMeta)> {
        self.get(&format!("/v1/peering/{}", name), opts).await
    }

    /// List all peerings
    pub async fn peering_list(&self, opts: &QueryOptions) -> Result<(Vec<Peering>, QueryMeta)> {
        self.get("/v1/peerings", opts).await
    }

    /// Delete a peering
    pub async fn peering_delete(
        &self,
        name: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        self.delete(&format!("/v1/peering/{}", name), opts, &[])
            .await
    }
}
