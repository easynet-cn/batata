use std::collections::HashMap;

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{
    CatalogDeregistration, CatalogNode, CatalogRegistration, CatalogService, Node, QueryMeta,
    QueryOptions, WriteMeta, WriteOptions,
};

/// Catalog API operations
impl ConsulClient {
    /// List known datacenters
    pub async fn catalog_datacenters(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<String>, QueryMeta)> {
        self.get("/v1/catalog/datacenters", opts).await
    }

    /// List nodes in the catalog
    pub async fn catalog_nodes(&self, opts: &QueryOptions) -> Result<(Vec<Node>, QueryMeta)> {
        self.get("/v1/catalog/nodes", opts).await
    }

    /// List services in the catalog (returns service name -> tags mapping)
    pub async fn catalog_services(
        &self,
        opts: &QueryOptions,
    ) -> Result<(HashMap<String, Vec<String>>, QueryMeta)> {
        self.get("/v1/catalog/services", opts).await
    }

    /// List nodes providing a specific service
    pub async fn catalog_service(
        &self,
        service: &str,
        tag: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<CatalogService>, QueryMeta)> {
        let path = format!("/v1/catalog/service/{}", service);
        let mut extra = Vec::new();
        if !tag.is_empty() {
            extra.push(("tag", tag.to_string()));
        }
        self.get_with_extra(&path, opts, &extra).await
    }

    /// Get a single node and its services
    pub async fn catalog_node(
        &self,
        node: &str,
        opts: &QueryOptions,
    ) -> Result<(Option<CatalogNode>, QueryMeta)> {
        let path = format!("/v1/catalog/node/{}", node);
        self.get_optional(&path, opts).await
    }

    /// Register an entity (node, service, check) in the catalog
    pub async fn catalog_register(
        &self,
        registration: &CatalogRegistration,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response_with_body("/v1/catalog/register", registration, opts)
            .await
    }

    /// Deregister an entity from the catalog
    pub async fn catalog_deregister(
        &self,
        deregistration: &CatalogDeregistration,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response_with_body("/v1/catalog/deregister", deregistration, opts)
            .await
    }

    /// List services for a specific node
    pub async fn catalog_node_services(
        &self,
        node: &str,
        opts: &QueryOptions,
    ) -> Result<(Option<CatalogNode>, QueryMeta)> {
        let path = format!("/v1/catalog/node-services/{}", node);
        self.get_optional(&path, opts).await
    }

    /// List nodes providing a specific service, filtering by multiple tags
    pub async fn catalog_service_multiple_tags(
        &self,
        service: &str,
        tags: &[&str],
        opts: &QueryOptions,
    ) -> Result<(Vec<CatalogService>, QueryMeta)> {
        let path = format!("/v1/catalog/service/{}", service);
        let mut extra = Vec::new();
        for tag in tags {
            extra.push(("tag", tag.to_string()));
        }
        self.get_with_extra(&path, opts, &extra).await
    }

    /// Get Connect-capable service nodes, filtering by multiple tags
    pub async fn catalog_connect_multiple_tags(
        &self,
        service: &str,
        tags: &[&str],
        opts: &QueryOptions,
    ) -> Result<(Vec<CatalogService>, QueryMeta)> {
        let path = format!("/v1/catalog/connect/{}", service);
        let mut extra = Vec::new();
        for tag in tags {
            extra.push(("tag", tag.to_string()));
        }
        self.get_with_extra(&path, opts, &extra).await
    }

    /// Get Connect-capable service nodes
    pub async fn catalog_connect(
        &self,
        service: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<CatalogService>, QueryMeta)> {
        self.get(&format!("/v1/catalog/connect/{}", service), opts)
            .await
    }

    /// Get gateway services
    pub async fn catalog_gateway_services(
        &self,
        gateway: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<serde_json::Value>, QueryMeta)> {
        self.get(&format!("/v1/catalog/gateway-services/{}", gateway), opts)
            .await
    }
}
