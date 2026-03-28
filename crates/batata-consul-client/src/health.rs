use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{HealthCheck, QueryMeta, QueryOptions, ServiceEntry};

/// Health API operations
impl ConsulClient {
    /// Get health checks for a node
    pub async fn health_node(
        &self,
        node: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<HealthCheck>, QueryMeta)> {
        let path = format!("/v1/health/node/{}", node);
        self.get(&path, opts).await
    }

    /// Get health checks for a service
    pub async fn health_checks(
        &self,
        service: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<HealthCheck>, QueryMeta)> {
        let path = format!("/v1/health/checks/{}", service);
        self.get(&path, opts).await
    }

    /// Get service instances with health info
    pub async fn health_service(
        &self,
        service: &str,
        tag: &str,
        passing_only: bool,
        opts: &QueryOptions,
    ) -> Result<(Vec<ServiceEntry>, QueryMeta)> {
        let path = format!("/v1/health/service/{}", service);
        let mut extra = Vec::new();
        if !tag.is_empty() {
            extra.push(("tag", tag.to_string()));
        }
        if passing_only {
            extra.push(("passing", String::new()));
        }
        self.get_with_extra(&path, opts, &extra).await
    }

    /// Get checks in a given state (any, passing, warning, critical)
    pub async fn health_state(
        &self,
        state: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<HealthCheck>, QueryMeta)> {
        let path = format!("/v1/health/state/{}", state);
        self.get(&path, opts).await
    }

    /// Get connect-capable service instances with health info
    pub async fn health_connect(
        &self,
        service: &str,
        tag: &str,
        passing_only: bool,
        opts: &QueryOptions,
    ) -> Result<(Vec<ServiceEntry>, QueryMeta)> {
        let path = format!("/v1/health/connect/{}", service);
        let mut extra = Vec::new();
        if !tag.is_empty() {
            extra.push(("tag", tag.to_string()));
        }
        if passing_only {
            extra.push(("passing", String::new()));
        }
        self.get_with_extra(&path, opts, &extra).await
    }

    /// Get service instances with health info, filtering by multiple tags
    pub async fn health_service_multiple_tags(
        &self,
        service: &str,
        tags: &[&str],
        passing_only: bool,
        opts: &QueryOptions,
    ) -> Result<(Vec<ServiceEntry>, QueryMeta)> {
        let path = format!("/v1/health/service/{}", service);
        let mut extra = Vec::new();
        for tag in tags {
            extra.push(("tag", tag.to_string()));
        }
        if passing_only {
            extra.push(("passing", String::new()));
        }
        self.get_with_extra(&path, opts, &extra).await
    }

    /// Get connect-capable service instances with health info, filtering by multiple tags
    pub async fn health_connect_multiple_tags(
        &self,
        service: &str,
        tags: &[&str],
        passing_only: bool,
        opts: &QueryOptions,
    ) -> Result<(Vec<ServiceEntry>, QueryMeta)> {
        let path = format!("/v1/health/connect/{}", service);
        let mut extra = Vec::new();
        for tag in tags {
            extra.push(("tag", tag.to_string()));
        }
        if passing_only {
            extra.push(("passing", String::new()));
        }
        self.get_with_extra(&path, opts, &extra).await
    }

    /// Get health for ingress gateway service
    pub async fn health_ingress(
        &self,
        service: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<ServiceEntry>, QueryMeta)> {
        self.get(&format!("/v1/health/ingress/{}", service), opts)
            .await
    }
}
