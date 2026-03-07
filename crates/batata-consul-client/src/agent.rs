use std::collections::HashMap;

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{
    AgentCheck, AgentMember, AgentService, AgentServiceRegistration, QueryMeta, QueryOptions,
    WriteMeta, WriteOptions,
};

/// Agent API operations
impl ConsulClient {
    /// List services registered with the local agent
    pub async fn agent_services(
        &self,
        opts: &QueryOptions,
    ) -> Result<(HashMap<String, AgentService>, QueryMeta)> {
        self.get("/v1/agent/services", opts).await
    }

    /// Get a single service by ID from the local agent
    pub async fn agent_service(
        &self,
        service_id: &str,
        opts: &QueryOptions,
    ) -> Result<(AgentService, QueryMeta)> {
        let path = format!("/v1/agent/service/{}", service_id);
        self.get(&path, opts).await
    }

    /// Register a service with the local agent
    pub async fn agent_service_register(
        &self,
        registration: &AgentServiceRegistration,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response_with_body("/v1/agent/service/register", registration, opts)
            .await
    }

    /// Deregister a service from the local agent
    pub async fn agent_service_deregister(
        &self,
        service_id: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let path = format!("/v1/agent/service/deregister/{}", service_id);
        self.put_no_response(&path, opts, &[]).await
    }

    /// Enable or disable maintenance mode for a service
    pub async fn agent_service_maintenance(
        &self,
        service_id: &str,
        enable: bool,
        reason: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let path = format!("/v1/agent/service/maintenance/{}", service_id);
        let mut params = vec![("enable".to_string(), enable.to_string())];
        if !reason.is_empty() {
            params.push(("reason".to_string(), reason.to_string()));
        }
        self.put_no_response(&path, opts, &params).await
    }

    /// List checks registered with the local agent
    pub async fn agent_checks(
        &self,
        opts: &QueryOptions,
    ) -> Result<(HashMap<String, AgentCheck>, QueryMeta)> {
        self.get("/v1/agent/checks", opts).await
    }

    /// List members of the gossip pool (serf)
    pub async fn agent_members(
        &self,
        wan: bool,
        opts: &QueryOptions,
    ) -> Result<(Vec<AgentMember>, QueryMeta)> {
        let mut extra = Vec::new();
        if wan {
            extra.push(("wan".to_string(), "1".to_string()));
        }
        self.get_with_extra("/v1/agent/members", opts, &extra).await
    }

    /// Get the local agent's configuration and member info
    pub async fn agent_self(&self, opts: &QueryOptions) -> Result<(serde_json::Value, QueryMeta)> {
        self.get("/v1/agent/self", opts).await
    }

    /// Join a cluster by address
    pub async fn agent_join(
        &self,
        address: &str,
        wan: bool,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let path = format!("/v1/agent/join/{}", address);
        let mut params = Vec::new();
        if wan {
            params.push(("wan".to_string(), "1".to_string()));
        }
        self.put_no_response(&path, opts, &params).await
    }

    /// Gracefully leave the cluster
    pub async fn agent_leave(&self, opts: &WriteOptions) -> Result<WriteMeta> {
        self.put_no_response("/v1/agent/leave", opts, &[]).await
    }

    /// Force remove a node from the cluster
    pub async fn agent_force_leave(&self, node: &str, opts: &WriteOptions) -> Result<WriteMeta> {
        let path = format!("/v1/agent/force-leave/{}", node);
        self.put_no_response(&path, opts, &[]).await
    }

    /// Enable or disable maintenance mode on the agent
    pub async fn agent_node_maintenance(
        &self,
        enable: bool,
        reason: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let mut params = vec![("enable".to_string(), enable.to_string())];
        if !reason.is_empty() {
            params.push(("reason".to_string(), reason.to_string()));
        }
        self.put_no_response("/v1/agent/maintenance", opts, &params)
            .await
    }

    /// Update the ACL token used by the agent
    pub async fn agent_update_token(&self, token: &str, opts: &WriteOptions) -> Result<WriteMeta> {
        let body = serde_json::json!({ "Token": token });
        self.put_no_response_with_body("/v1/agent/token/default", &body, opts)
            .await
    }
}

// Helper: PUT with JSON body but no response body
impl ConsulClient {
    pub(crate) async fn put_no_response_with_body<B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        use crate::error::ConsulError;

        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        let mut req = self.client.put(&url).json(body);
        if !token.is_empty() {
            req = req.header("X-Consul-Token", token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }

        let response = req.send().await.map_err(ConsulError::Http)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        Ok(WriteMeta {
            request_time: start.elapsed(),
        })
    }
}
