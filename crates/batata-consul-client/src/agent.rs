use std::collections::HashMap;

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{
    AgentCheck, AgentCheckRegistration, AgentCheckUpdate, AgentMember, AgentService,
    AgentServiceRegistration, QueryMeta, QueryOptions, WriteMeta, WriteOptions,
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

    /// Register a health check
    pub async fn agent_check_register(
        &self,
        check: &AgentCheckRegistration,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response_with_body("/v1/agent/check/register", check, opts)
            .await
    }

    /// Deregister a health check
    pub async fn agent_check_deregister(
        &self,
        check_id: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response(
            &format!("/v1/agent/check/deregister/{}", check_id),
            opts,
            &[],
        )
        .await
    }

    /// Mark a TTL check as passing
    pub async fn agent_check_pass(
        &self,
        check_id: &str,
        note: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = if note.is_empty() {
            vec![]
        } else {
            vec![("note".to_string(), note.to_string())]
        };
        self.put_no_response(&format!("/v1/agent/check/pass/{}", check_id), opts, &extra)
            .await
    }

    /// Mark a TTL check as warning
    pub async fn agent_check_warn(
        &self,
        check_id: &str,
        note: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = if note.is_empty() {
            vec![]
        } else {
            vec![("note".to_string(), note.to_string())]
        };
        self.put_no_response(&format!("/v1/agent/check/warn/{}", check_id), opts, &extra)
            .await
    }

    /// Mark a TTL check as critical
    pub async fn agent_check_fail(
        &self,
        check_id: &str,
        note: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = if note.is_empty() {
            vec![]
        } else {
            vec![("note".to_string(), note.to_string())]
        };
        self.put_no_response(&format!("/v1/agent/check/fail/{}", check_id), opts, &extra)
            .await
    }

    /// Update a TTL check with custom status and output
    pub async fn agent_check_update(
        &self,
        check_id: &str,
        update: &AgentCheckUpdate,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response_with_body(
            &format!("/v1/agent/check/update/{}", check_id),
            update,
            opts,
        )
        .await
    }

    /// Enable node maintenance mode
    pub async fn agent_enable_node_maintenance(
        &self,
        reason: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = vec![
            ("enable".to_string(), "true".to_string()),
            ("reason".to_string(), reason.to_string()),
        ];
        self.put_no_response("/v1/agent/maintenance", opts, &extra)
            .await
    }

    /// Disable node maintenance mode
    pub async fn agent_disable_node_maintenance(&self, opts: &WriteOptions) -> Result<WriteMeta> {
        let extra = vec![("enable".to_string(), "false".to_string())];
        self.put_no_response("/v1/agent/maintenance", opts, &extra)
            .await
    }

    /// Enable service maintenance mode
    pub async fn agent_enable_service_maintenance(
        &self,
        service_id: &str,
        reason: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = vec![
            ("enable".to_string(), "true".to_string()),
            ("reason".to_string(), reason.to_string()),
        ];
        self.put_no_response(
            &format!("/v1/agent/service/maintenance/{}", service_id),
            opts,
            &extra,
        )
        .await
    }

    /// Disable service maintenance mode
    pub async fn agent_disable_service_maintenance(
        &self,
        service_id: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = vec![("enable".to_string(), "false".to_string())];
        self.put_no_response(
            &format!("/v1/agent/service/maintenance/{}", service_id),
            opts,
            &extra,
        )
        .await
    }

    /// Get agent host information
    pub async fn agent_host(&self, opts: &QueryOptions) -> Result<(serde_json::Value, QueryMeta)> {
        self.get("/v1/agent/host", opts).await
    }

    /// Get agent version
    pub async fn agent_version(
        &self,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get("/v1/agent/version", opts).await
    }

    /// Get agent metrics
    pub async fn agent_metrics(
        &self,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get("/v1/agent/metrics", opts).await
    }

    /// Reload agent configuration
    pub async fn agent_reload(&self, opts: &WriteOptions) -> Result<WriteMeta> {
        self.put_no_response("/v1/agent/reload", opts, &[]).await
    }

    /// Get service health by ID
    pub async fn agent_health_service_by_id(
        &self,
        service_id: &str,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get(&format!("/v1/agent/health/service/id/{}", service_id), opts)
            .await
    }

    /// Get service health by name
    pub async fn agent_health_service_by_name(
        &self,
        service_name: &str,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get(
            &format!("/v1/agent/health/service/name/{}", service_name),
            opts,
        )
        .await
    }

    /// Get Connect CA roots via agent
    pub async fn agent_connect_ca_roots(
        &self,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get("/v1/agent/connect/ca/roots", opts).await
    }

    /// Get Connect leaf certificate for a service
    pub async fn agent_connect_ca_leaf(
        &self,
        service: &str,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get(&format!("/v1/agent/connect/ca/leaf/{}", service), opts)
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
