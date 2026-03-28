use std::collections::HashMap;

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{
    ACLAuthMethod, ACLAuthMethodListEntry, ACLBindingRule, ACLLoginParams, ACLOIDCAuthURLParams,
    ACLOIDCCallbackParams, ACLPolicy, ACLPolicyListEntry, ACLReplicationStatus, ACLRole,
    ACLTemplatedPolicy, ACLTemplatedPolicyResponse, ACLToken, ACLTokenFilterOptions,
    ACLTokenListEntry, QueryMeta, QueryOptions, WriteMeta, WriteOptions,
};

/// ACL API operations
impl ConsulClient {
    // --- Bootstrap ---

    /// Bootstrap the ACL system, creating the initial management token
    pub async fn acl_bootstrap(&self, opts: &WriteOptions) -> Result<(ACLToken, WriteMeta)> {
        self.put::<ACLToken, ()>("/v1/acl/bootstrap", None, opts, &[])
            .await
    }

    // --- Token Operations ---

    /// Create a new ACL token
    pub async fn acl_token_create(
        &self,
        token: &ACLToken,
        opts: &WriteOptions,
    ) -> Result<(ACLToken, WriteMeta)> {
        self.put("/v1/acl/token", Some(token), opts, &[]).await
    }

    /// Update an existing ACL token
    pub async fn acl_token_update(
        &self,
        token: &ACLToken,
        opts: &WriteOptions,
    ) -> Result<(ACLToken, WriteMeta)> {
        let path = format!("/v1/acl/token/{}", token.accessor_id);
        self.put(&path, Some(token), opts, &[]).await
    }

    /// Clone an existing ACL token
    pub async fn acl_token_clone(
        &self,
        accessor_id: &str,
        description: &str,
        opts: &WriteOptions,
    ) -> Result<(ACLToken, WriteMeta)> {
        let path = format!("/v1/acl/token/{}/clone", accessor_id);
        let body = serde_json::json!({ "Description": description });
        self.put(&path, Some(&body), opts, &[]).await
    }

    /// Delete an ACL token
    pub async fn acl_token_delete(
        &self,
        accessor_id: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/acl/token/{}", accessor_id);
        self.delete(&path, opts, &[]).await
    }

    /// Read an ACL token by accessor ID
    pub async fn acl_token_read(
        &self,
        accessor_id: &str,
        opts: &QueryOptions,
    ) -> Result<(ACLToken, QueryMeta)> {
        let path = format!("/v1/acl/token/{}", accessor_id);
        self.get(&path, opts).await
    }

    /// Read the token associated with the current request (self-token)
    pub async fn acl_token_read_self(&self, opts: &QueryOptions) -> Result<(ACLToken, QueryMeta)> {
        self.get("/v1/acl/token/self", opts).await
    }

    /// List all ACL tokens
    pub async fn acl_token_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<ACLTokenListEntry>, QueryMeta)> {
        self.get("/v1/acl/tokens", opts).await
    }

    /// List ACL tokens with filters
    pub async fn acl_token_list_filtered(
        &self,
        filter_opts: &ACLTokenFilterOptions,
        opts: &QueryOptions,
    ) -> Result<(Vec<ACLTokenListEntry>, QueryMeta)> {
        let mut extra = Vec::new();
        if let Some(ref am) = filter_opts.auth_method {
            extra.push(("authmethod", am.clone()));
        }
        if let Some(ref policy) = filter_opts.policy {
            extra.push(("policy", policy.clone()));
        }
        if let Some(ref role) = filter_opts.role {
            extra.push(("role", role.clone()));
        }
        if let Some(ref svc) = filter_opts.service_name {
            extra.push(("servicename", svc.clone()));
        }
        self.get_with_extra("/v1/acl/tokens", opts, &extra).await
    }

    // --- Policy Operations ---

    /// Create a new ACL policy
    pub async fn acl_policy_create(
        &self,
        policy: &ACLPolicy,
        opts: &WriteOptions,
    ) -> Result<(ACLPolicy, WriteMeta)> {
        self.put("/v1/acl/policy", Some(policy), opts, &[]).await
    }

    /// Update an existing ACL policy
    pub async fn acl_policy_update(
        &self,
        policy: &ACLPolicy,
        opts: &WriteOptions,
    ) -> Result<(ACLPolicy, WriteMeta)> {
        let path = format!("/v1/acl/policy/{}", policy.id);
        self.put(&path, Some(policy), opts, &[]).await
    }

    /// Delete an ACL policy
    pub async fn acl_policy_delete(
        &self,
        policy_id: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/acl/policy/{}", policy_id);
        self.delete(&path, opts, &[]).await
    }

    /// Read an ACL policy by ID
    pub async fn acl_policy_read(
        &self,
        policy_id: &str,
        opts: &QueryOptions,
    ) -> Result<(ACLPolicy, QueryMeta)> {
        let path = format!("/v1/acl/policy/{}", policy_id);
        self.get(&path, opts).await
    }

    /// Read an ACL policy by name
    pub async fn acl_policy_read_by_name(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(ACLPolicy, QueryMeta)> {
        let path = format!("/v1/acl/policy/name/{}", name);
        self.get(&path, opts).await
    }

    /// List all ACL policies
    pub async fn acl_policy_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<ACLPolicyListEntry>, QueryMeta)> {
        self.get("/v1/acl/policies", opts).await
    }

    // --- Role Operations ---

    /// Create a new ACL role
    pub async fn acl_role_create(
        &self,
        role: &ACLRole,
        opts: &WriteOptions,
    ) -> Result<(ACLRole, WriteMeta)> {
        self.put("/v1/acl/role", Some(role), opts, &[]).await
    }

    /// Update an existing ACL role
    pub async fn acl_role_update(
        &self,
        role: &ACLRole,
        opts: &WriteOptions,
    ) -> Result<(ACLRole, WriteMeta)> {
        let path = format!("/v1/acl/role/{}", role.id);
        self.put(&path, Some(role), opts, &[]).await
    }

    /// Delete an ACL role
    pub async fn acl_role_delete(
        &self,
        role_id: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/acl/role/{}", role_id);
        self.delete(&path, opts, &[]).await
    }

    /// Read an ACL role by ID
    pub async fn acl_role_read(
        &self,
        role_id: &str,
        opts: &QueryOptions,
    ) -> Result<(ACLRole, QueryMeta)> {
        let path = format!("/v1/acl/role/{}", role_id);
        self.get(&path, opts).await
    }

    /// Read an ACL role by name
    pub async fn acl_role_read_by_name(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(ACLRole, QueryMeta)> {
        let path = format!("/v1/acl/role/name/{}", name);
        self.get(&path, opts).await
    }

    /// List all ACL roles
    pub async fn acl_role_list(&self, opts: &QueryOptions) -> Result<(Vec<ACLRole>, QueryMeta)> {
        self.get("/v1/acl/roles", opts).await
    }

    // --- Auth Method Operations ---

    /// Create a new ACL auth method
    pub async fn acl_auth_method_create(
        &self,
        method: &ACLAuthMethod,
        opts: &WriteOptions,
    ) -> Result<(ACLAuthMethod, WriteMeta)> {
        self.put("/v1/acl/auth-method", Some(method), opts, &[])
            .await
    }

    /// Update an existing ACL auth method
    pub async fn acl_auth_method_update(
        &self,
        method: &ACLAuthMethod,
        opts: &WriteOptions,
    ) -> Result<(ACLAuthMethod, WriteMeta)> {
        let path = format!("/v1/acl/auth-method/{}", method.name);
        self.put(&path, Some(method), opts, &[]).await
    }

    /// Delete an ACL auth method
    pub async fn acl_auth_method_delete(
        &self,
        name: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/acl/auth-method/{}", name);
        self.delete(&path, opts, &[]).await
    }

    /// Read an ACL auth method by name
    pub async fn acl_auth_method_read(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(ACLAuthMethod, QueryMeta)> {
        let path = format!("/v1/acl/auth-method/{}", name);
        self.get(&path, opts).await
    }

    /// List all ACL auth methods
    pub async fn acl_auth_method_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<ACLAuthMethodListEntry>, QueryMeta)> {
        self.get("/v1/acl/auth-methods", opts).await
    }

    // --- Binding Rule Operations ---

    /// Create a new ACL binding rule
    pub async fn acl_binding_rule_create(
        &self,
        rule: &ACLBindingRule,
        opts: &WriteOptions,
    ) -> Result<(ACLBindingRule, WriteMeta)> {
        self.put("/v1/acl/binding-rule", Some(rule), opts, &[])
            .await
    }

    /// Update an existing ACL binding rule
    pub async fn acl_binding_rule_update(
        &self,
        rule: &ACLBindingRule,
        opts: &WriteOptions,
    ) -> Result<(ACLBindingRule, WriteMeta)> {
        let path = format!("/v1/acl/binding-rule/{}", rule.id);
        self.put(&path, Some(rule), opts, &[]).await
    }

    /// Delete an ACL binding rule
    pub async fn acl_binding_rule_delete(
        &self,
        rule_id: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/acl/binding-rule/{}", rule_id);
        self.delete(&path, opts, &[]).await
    }

    /// Read an ACL binding rule by ID
    pub async fn acl_binding_rule_read(
        &self,
        rule_id: &str,
        opts: &QueryOptions,
    ) -> Result<(ACLBindingRule, QueryMeta)> {
        let path = format!("/v1/acl/binding-rule/{}", rule_id);
        self.get(&path, opts).await
    }

    /// List ACL binding rules, optionally filtered by auth method
    pub async fn acl_binding_rule_list(
        &self,
        auth_method: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<ACLBindingRule>, QueryMeta)> {
        let mut extra = Vec::new();
        if !auth_method.is_empty() {
            extra.push(("authmethod", auth_method.to_string()));
        }
        self.get_with_extra("/v1/acl/binding-rules", opts, &extra)
            .await
    }

    // --- Login / Logout ---

    /// Login to ACL using an auth method
    pub async fn acl_login(
        &self,
        params: &ACLLoginParams,
        opts: &WriteOptions,
    ) -> Result<(ACLToken, WriteMeta)> {
        self.post("/v1/acl/login", Some(params), opts, &[]).await
    }

    /// Logout from ACL, destroying the token created by login
    pub async fn acl_logout(&self, opts: &WriteOptions) -> Result<WriteMeta> {
        self.put_no_response("/v1/acl/logout", opts, &[]).await
    }

    // --- OIDC ---

    /// Get an OIDC authorization URL
    pub async fn acl_oidc_auth_url(
        &self,
        params: &ACLOIDCAuthURLParams,
        opts: &WriteOptions,
    ) -> Result<(serde_json::Value, WriteMeta)> {
        self.post("/v1/acl/oidc/auth-url", Some(params), opts, &[])
            .await
    }

    /// Exchange an OIDC authorization code for a token
    pub async fn acl_oidc_callback(
        &self,
        params: &ACLOIDCCallbackParams,
        opts: &WriteOptions,
    ) -> Result<(ACLToken, WriteMeta)> {
        self.post("/v1/acl/oidc/callback", Some(params), opts, &[])
            .await
    }

    // --- Replication ---

    /// Get ACL replication status
    pub async fn acl_replication(
        &self,
        opts: &QueryOptions,
    ) -> Result<(ACLReplicationStatus, QueryMeta)> {
        self.get("/v1/acl/replication", opts).await
    }

    // --- Templated Policies ---

    /// List all templated policies
    pub async fn acl_templated_policy_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(HashMap<String, ACLTemplatedPolicyResponse>, QueryMeta)> {
        self.get("/v1/acl/templated-policies", opts).await
    }

    /// Read a templated policy by name
    pub async fn acl_templated_policy_read(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(ACLTemplatedPolicyResponse, QueryMeta)> {
        let path = format!("/v1/acl/templated-policy/name/{}", name);
        self.get(&path, opts).await
    }

    /// Preview a rendered templated policy
    pub async fn acl_templated_policy_preview(
        &self,
        tp: &ACLTemplatedPolicy,
        opts: &WriteOptions,
    ) -> Result<(ACLPolicy, WriteMeta)> {
        self.post(
            "/v1/acl/templated-policy/preview/{name}",
            Some(tp),
            opts,
            &[],
        )
        .await
    }

    /// Read an ACL token by accessor ID with expanded information.
    ///
    /// This returns the token along with expanded details about its policies,
    /// roles, and other associated objects. Uses the `?expanded=true` query parameter.
    pub async fn acl_token_read_expanded(
        &self,
        accessor_id: &str,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        let path = format!("/v1/acl/token/{}", accessor_id);
        let extra = vec![("expanded", "true".to_string())];
        self.get_with_extra(&path, opts, &extra).await
    }

    /// Translate legacy ACL rules to the current syntax.
    ///
    /// NOTE: Legacy ACL rules were deprecated in Consul 1.4.
    /// This method always returns an error for compatibility.
    pub async fn acl_rules_translate(&self) -> Result<String> {
        Err(crate::error::ConsulError::Other(
            "Legacy ACL rules were deprecated in Consul 1.4".to_string(),
        ))
    }

    /// Bootstrap the ACL system with a specific bootstrap token.
    ///
    /// Unlike `acl_bootstrap()`, this allows specifying the initial
    /// secret to use as the bootstrap token.
    pub async fn acl_bootstrap_with_token(
        &self,
        bootstrap_secret: &str,
        opts: &WriteOptions,
    ) -> Result<(ACLToken, WriteMeta)> {
        let body = serde_json::json!({ "BootstrapSecret": bootstrap_secret });
        self.put("/v1/acl/bootstrap", Some(&body), opts, &[]).await
    }
}
