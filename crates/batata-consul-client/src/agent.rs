use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::client::ConsulClient;
use crate::error::{ConsulError, Result};
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
        let mut params = vec![("enable", enable.to_string())];
        if !reason.is_empty() {
            params.push(("reason", reason.to_string()));
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
            extra.push(("wan", "1".to_string()));
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
            params.push(("wan", "1".to_string()));
        }
        self.put_no_response(&path, opts, &params).await
    }

    /// Gracefully leave the cluster
    pub async fn agent_leave(&self, opts: &WriteOptions) -> Result<WriteMeta> {
        self.put_no_response("/v1/agent/leave", opts, &[]).await
    }

    /// Force remove a node from the cluster.
    pub async fn agent_force_leave(&self, node: &str, opts: &WriteOptions) -> Result<WriteMeta> {
        let path = format!("/v1/agent/force-leave/{}", node);
        self.put_no_response(&path, opts, &[]).await
    }

    /// Force remove a node with additional options (`prune` = fully remove
    /// vs. mark-as-leaving). Matches Go SDK `ForceLeaveOptions`.
    pub async fn agent_force_leave_options(
        &self,
        node: &str,
        prune: bool,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let path = format!("/v1/agent/force-leave/{}", node);
        let mut extra = Vec::new();
        if prune {
            extra.push(("prune", String::new()));
        }
        self.put_no_response(&path, opts, &extra).await
    }

    /// Convenience: force-leave with `prune=true` (Go SDK `ForceLeavePrune`).
    pub async fn agent_force_leave_prune(
        &self,
        node: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.agent_force_leave_options(node, true, opts).await
    }

    /// Enable or disable maintenance mode on the agent
    pub async fn agent_node_maintenance(
        &self,
        enable: bool,
        reason: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let mut params = vec![("enable", enable.to_string())];
        if !reason.is_empty() {
            params.push(("reason", reason.to_string()));
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
            vec![("note", note.to_string())]
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
            vec![("note", note.to_string())]
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
            vec![("note", note.to_string())]
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
            ("enable", "true".to_string()),
            ("reason", reason.to_string()),
        ];
        self.put_no_response("/v1/agent/maintenance", opts, &extra)
            .await
    }

    /// Disable node maintenance mode
    pub async fn agent_disable_node_maintenance(&self, opts: &WriteOptions) -> Result<WriteMeta> {
        let extra = vec![("enable", "false".to_string())];
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
            ("enable", "true".to_string()),
            ("reason", reason.to_string()),
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
        let extra = vec![("enable", "false".to_string())];
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

    // ========== ACL Token Update Variants ==========
    // These match the Consul Go client's UpdateACL*Token methods.
    // Each updates a different type of agent ACL token.

    /// Update the default ACL token used by the agent
    pub async fn agent_update_default_acl_token(
        &self,
        token: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let body = serde_json::json!({ "Token": token });
        self.put_no_response_with_body("/v1/agent/token/default", &body, opts)
            .await
    }

    /// Update the agent ACL token (used for internal agent operations)
    pub async fn agent_update_agent_acl_token(
        &self,
        token: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let body = serde_json::json!({ "Token": token });
        self.put_no_response_with_body("/v1/agent/token/agent", &body, opts)
            .await
    }

    /// Update the agent recovery ACL token
    pub async fn agent_update_agent_recovery_acl_token(
        &self,
        token: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let body = serde_json::json!({ "Token": token });
        self.put_no_response_with_body("/v1/agent/token/agent_recovery", &body, opts)
            .await
    }

    /// Update the replication ACL token
    pub async fn agent_update_replication_acl_token(
        &self,
        token: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let body = serde_json::json!({ "Token": token });
        self.put_no_response_with_body("/v1/agent/token/replication", &body, opts)
            .await
    }

    /// Update the config file registration ACL token
    pub async fn agent_update_config_file_registration_token(
        &self,
        token: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let body = serde_json::json!({ "Token": token });
        self.put_no_response_with_body(
            "/v1/agent/token/config_file_service_registration",
            &body,
            opts,
        )
        .await
    }

    /// Update the DNS ACL token
    pub async fn agent_update_dns_token(
        &self,
        token: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let body = serde_json::json!({ "Token": token });
        self.put_no_response_with_body("/v1/agent/token/dns", &body, opts)
            .await
    }

    /// Get the node name of the local agent.
    ///
    /// Calls the `/v1/agent/self` endpoint and extracts the `NodeName`
    /// from the `Config` section of the response.
    pub async fn agent_node_name(&self, opts: &QueryOptions) -> Result<String> {
        let (info, _): (serde_json::Value, QueryMeta) = self.get("/v1/agent/self", opts).await?;
        let name = info
            .get("Config")
            .and_then(|c| c.get("NodeName"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        Ok(name)
    }

    /// List services registered with the local agent, filtered by a filter expression.
    ///
    /// The filter is applied server-side using Consul's filtering syntax.
    pub async fn agent_services_with_filter(
        &self,
        filter: &str,
        opts: &QueryOptions,
    ) -> Result<(HashMap<String, AgentService>, QueryMeta)> {
        let mut extra = Vec::new();
        if !filter.is_empty() {
            extra.push(("filter", filter.to_string()));
        }
        self.get_with_extra("/v1/agent/services", opts, &extra)
            .await
    }

    /// List checks registered with the local agent, filtered by a filter expression.
    ///
    /// The filter is applied server-side using Consul's filtering syntax.
    pub async fn agent_checks_with_filter(
        &self,
        filter: &str,
        opts: &QueryOptions,
    ) -> Result<(HashMap<String, AgentCheck>, QueryMeta)> {
        let mut extra = Vec::new();
        if !filter.is_empty() {
            extra.push(("filter", filter.to_string()));
        }
        self.get_with_extra("/v1/agent/checks", opts, &extra).await
    }

    /// Stream agent logs at the given log level.
    ///
    /// Returns an mpsc receiver that yields log lines. The streaming continues
    /// until the response stream ends or the receiver is dropped.
    /// This calls `GET /v1/agent/monitor?loglevel=<level>` which returns a
    /// streaming response.
    pub async fn agent_monitor(
        &self,
        log_level: &str,
        opts: &QueryOptions,
    ) -> Result<(mpsc::Receiver<String>, tokio::task::JoinHandle<()>)> {
        let mut params = Vec::new();
        self.apply_query_options(&mut params, opts);
        if !log_level.is_empty() {
            params.push(("loglevel", log_level.to_string()));
        }

        let token = self.effective_token(&opts.token);
        let url = self.url("/v1/agent/monitor");

        let mut req = self.client.get(&url);
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

        let (tx, rx) = mpsc::channel(64);
        let handle = tokio::spawn(async move {
            use futures::StreamExt;
            let mut stream = response.bytes_stream();

            let mut buffer = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            buffer.push_str(&text);
                            // Process complete lines
                            while let Some(pos) = buffer.find('\n') {
                                let line = buffer[..pos].trim_end().to_string();
                                buffer = buffer[pos + 1..].to_string();
                                if !line.is_empty() && tx.send(line).await.is_err() {
                                    return; // receiver dropped
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            // Flush remaining buffer
            let line = buffer.trim_end().to_string();
            if !line.is_empty() {
                let _ = tx.send(line).await;
            }
        });

        Ok((rx, handle))
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

    /// Stream agent logs as JSON lines (Go SDK `Agent.MonitorJSON`).
    /// Identical to `agent_monitor` but requests `logjson=true`.
    pub async fn agent_monitor_json(
        &self,
        log_level: &str,
        opts: &QueryOptions,
    ) -> Result<(mpsc::Receiver<String>, tokio::task::JoinHandle<()>)> {
        let mut params = Vec::new();
        self.apply_query_options(&mut params, opts);
        if !log_level.is_empty() {
            params.push(("loglevel", log_level.to_string()));
        }
        params.push(("logjson", "true".to_string()));

        let token = self.effective_token(&opts.token);
        let url = self.url("/v1/agent/monitor");
        let mut req = self.client.get(&url);
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

        let (tx, rx) = mpsc::channel(64);
        let handle = tokio::spawn(async move {
            use futures::StreamExt;
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            buffer.push_str(&text);
                            while let Some(pos) = buffer.find('\n') {
                                let line = buffer[..pos].trim_end().to_string();
                                buffer.drain(..=pos);
                                if !line.is_empty() && tx.send(line).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            if !buffer.is_empty() {
                let _ = tx.send(buffer).await;
            }
        });
        Ok((rx, handle))
    }

    /// Check health of a service by service ID on the local agent, returning
    /// the AgentServiceChecksInfo struct (Go SDK `AgentHealthServiceByID`).
    ///
    /// Returns a tuple `(status, info)` where `status` is "passing", "warning",
    /// "critical", or "" (unknown). `info` is None when the service doesn't
    /// exist on this agent.
    pub async fn agent_health_service_by_id_opts(
        &self,
        service_id: &str,
        opts: &QueryOptions,
    ) -> Result<(String, Option<serde_json::Value>)> {
        let path = format!("/v1/agent/health/service/id/{}", service_id);
        match self.get::<serde_json::Value>(&path, opts).await {
            Ok((v, _meta)) => {
                let status = v
                    .get("AggregatedStatus")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok((status, Some(v)))
            }
            Err(e) if e.is_not_found() => Ok((String::new(), None)),
            Err(e) => Err(e),
        }
    }

    /// Check health of all services with a given name on the local agent,
    /// Go SDK `AgentHealthServiceByName`.
    pub async fn agent_health_service_by_name_opts(
        &self,
        service_name: &str,
        opts: &QueryOptions,
    ) -> Result<(String, Vec<serde_json::Value>)> {
        let path = format!("/v1/agent/health/service/name/{}", service_name);
        match self.get::<Vec<serde_json::Value>>(&path, opts).await {
            Ok((infos, _meta)) => {
                // Aggregate status across instances: critical > warning > passing
                let mut status = "";
                for info in &infos {
                    if let Some(s) = info.get("AggregatedStatus").and_then(|s| s.as_str()) {
                        if s == "critical" {
                            status = "critical";
                            break;
                        }
                        if s == "warning" && status != "critical" {
                            status = "warning";
                        } else if s == "passing" && status.is_empty() {
                            status = "passing";
                        }
                    }
                }
                Ok((status.to_string(), infos))
            }
            Err(e) if e.is_not_found() => Ok((String::new(), Vec::new())),
            Err(e) => Err(e),
        }
    }

    /// Update the agent's "agent recovery" ACL token.
    ///
    /// Maps to Go SDK `Agent.UpdateAgentRecoveryACLToken` (post 1.11 rename).
    pub async fn agent_update_agent_recovery_token(
        &self,
        token: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.agent_update_token_of_type("agent_recovery", token, opts)
            .await
    }

    /// Generic helper: update an ACL token of the specified type.
    /// Token types: "default", "agent", "agent_recovery", "replication",
    /// "config_file_service_registration", "dns".
    pub async fn agent_update_token_of_type(
        &self,
        token_type: &str,
        token: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let path = format!("/v1/agent/token/{}", token_type);
        let body = serde_json::json!({ "Token": token });
        let (_resp, meta): (serde_json::Value, WriteMeta) =
            self.put(&path, Some(&body), opts, &[]).await?;
        Ok(meta)
    }
}
