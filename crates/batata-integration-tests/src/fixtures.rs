//! Test fixtures and builders for integration testing
//!
//! Provides pre-configured test data builders to reduce boilerplate in tests.

use std::collections::HashMap;

/// Builder for configuration test data
pub struct ConfigFixture {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub config_type: String,
    pub desc: String,
    pub app_name: String,
    pub tags: String,
    pub encrypted_data_key: String,
}

impl Default for ConfigFixture {
    fn default() -> Self {
        Self {
            data_id: format!("test-config-{}", super::unique_test_id()),
            group: "DEFAULT_GROUP".to_string(),
            tenant: "".to_string(),
            content: "key=value".to_string(),
            config_type: "properties".to_string(),
            desc: "Test configuration".to_string(),
            app_name: "".to_string(),
            tags: "".to_string(),
            encrypted_data_key: "".to_string(),
        }
    }
}

impl ConfigFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_data_id(mut self, data_id: &str) -> Self {
        self.data_id = data_id.to_string();
        self
    }

    pub fn with_group(mut self, group: &str) -> Self {
        self.group = group.to_string();
        self
    }

    pub fn with_tenant(mut self, tenant: &str) -> Self {
        self.tenant = tenant.to_string();
        self
    }

    pub fn with_content(mut self, content: &str) -> Self {
        self.content = content.to_string();
        self
    }

    pub fn with_type(mut self, config_type: &str) -> Self {
        self.config_type = config_type.to_string();
        self
    }

    pub fn with_desc(mut self, desc: &str) -> Self {
        self.desc = desc.to_string();
        self
    }

    pub fn with_app_name(mut self, app_name: &str) -> Self {
        self.app_name = app_name.to_string();
        self
    }

    pub fn with_tags(mut self, tags: &str) -> Self {
        self.tags = tags.to_string();
        self
    }

    pub fn with_encryption(mut self, data_key: &str) -> Self {
        self.encrypted_data_key = data_key.to_string();
        self
    }

    pub fn yaml(mut self) -> Self {
        self.config_type = "yaml".to_string();
        self.content = "server:\n  port: 8080\n  name: test".to_string();
        self.data_id = format!("test-config-{}.yaml", super::unique_test_id());
        self
    }

    pub fn json(mut self) -> Self {
        self.config_type = "json".to_string();
        self.content = r#"{"server":{"port":8080}}"#.to_string();
        self.data_id = format!("test-config-{}.json", super::unique_test_id());
        self
    }

    pub fn properties(mut self) -> Self {
        self.config_type = "properties".to_string();
        self.content = "server.port=8080\nserver.name=test".to_string();
        self.data_id = format!("test-config-{}.properties", super::unique_test_id());
        self
    }

    /// Convert to form params for V2 API
    pub fn to_v2_publish_params(&self) -> Vec<(&str, &str)> {
        let mut params: Vec<(&str, &str)> = vec![
            ("dataId", &self.data_id),
            ("group", &self.group),
            ("content", &self.content),
            ("type", &self.config_type),
        ];
        if !self.tenant.is_empty() {
            params.push(("tenant", &self.tenant));
        }
        if !self.desc.is_empty() {
            params.push(("desc", &self.desc));
        }
        if !self.app_name.is_empty() {
            params.push(("appName", &self.app_name));
        }
        if !self.tags.is_empty() {
            params.push(("config_tags", &self.tags));
        }
        params
    }

    /// Convert to query params for V2 API
    pub fn to_v2_query_params(&self) -> Vec<(&str, &str)> {
        let mut params: Vec<(&str, &str)> = vec![("dataId", &self.data_id), ("group", &self.group)];
        if !self.tenant.is_empty() {
            params.push(("tenant", &self.tenant));
        }
        params
    }
}

/// Builder for service instance test data
pub struct InstanceFixture {
    pub service_name: String,
    pub group_name: String,
    pub namespace_id: String,
    pub ip: String,
    pub port: u16,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub ephemeral: bool,
    pub cluster_name: String,
    pub metadata: HashMap<String, String>,
}

impl Default for InstanceFixture {
    fn default() -> Self {
        Self {
            service_name: format!("test-service-{}", super::unique_test_id()),
            group_name: "DEFAULT_GROUP".to_string(),
            namespace_id: "".to_string(),
            ip: "127.0.0.1".to_string(),
            port: 8080,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            metadata: HashMap::new(),
        }
    }
}

impl InstanceFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_service_name(mut self, name: &str) -> Self {
        self.service_name = name.to_string();
        self
    }

    pub fn with_group(mut self, group: &str) -> Self {
        self.group_name = group.to_string();
        self
    }

    pub fn with_namespace(mut self, ns: &str) -> Self {
        self.namespace_id = ns.to_string();
        self
    }

    pub fn with_ip(mut self, ip: &str) -> Self {
        self.ip = ip.to_string();
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    pub fn unhealthy(mut self) -> Self {
        self.healthy = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn persistent(mut self) -> Self {
        self.ephemeral = false;
        self
    }

    pub fn with_cluster(mut self, cluster: &str) -> Self {
        self.cluster_name = cluster.to_string();
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Convert to form params for V2 API registration
    pub fn to_v2_register_params(&self) -> Vec<(String, String)> {
        let mut params = vec![
            ("serviceName".to_string(), self.service_name.clone()),
            ("groupName".to_string(), self.group_name.clone()),
            ("ip".to_string(), self.ip.clone()),
            ("port".to_string(), self.port.to_string()),
            ("weight".to_string(), self.weight.to_string()),
            ("healthy".to_string(), self.healthy.to_string()),
            ("enabled".to_string(), self.enabled.to_string()),
            ("ephemeral".to_string(), self.ephemeral.to_string()),
            ("clusterName".to_string(), self.cluster_name.clone()),
        ];
        if !self.namespace_id.is_empty() {
            params.push(("namespaceId".to_string(), self.namespace_id.clone()));
        }
        if !self.metadata.is_empty() {
            let meta_json = serde_json::to_string(&self.metadata).unwrap_or_default();
            params.push(("metadata".to_string(), meta_json));
        }
        params
    }
}

/// Builder for namespace test data
pub struct NamespaceFixture {
    pub namespace_id: String,
    pub namespace_name: String,
    pub namespace_desc: String,
}

impl Default for NamespaceFixture {
    fn default() -> Self {
        Self {
            namespace_id: format!("test-ns-{}", super::unique_test_id()),
            namespace_name: "Test Namespace".to_string(),
            namespace_desc: "Namespace for testing".to_string(),
        }
    }
}

impl NamespaceFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.namespace_id = id.to_string();
        self
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.namespace_name = name.to_string();
        self
    }

    pub fn with_desc(mut self, desc: &str) -> Self {
        self.namespace_desc = desc.to_string();
        self
    }

    pub fn to_create_params(&self) -> Vec<(&str, &str)> {
        vec![
            ("customNamespaceId", &self.namespace_id),
            ("namespaceName", &self.namespace_name),
            ("namespaceDesc", &self.namespace_desc),
        ]
    }
}

/// Builder for auth test data
pub struct AuthFixture {
    pub username: String,
    pub password: String,
    pub role: String,
}

impl Default for AuthFixture {
    fn default() -> Self {
        Self {
            username: format!("testuser_{}", super::unique_test_id()),
            password: "TestPassword123!".to_string(),
            role: "ROLE_DEVELOPER".to_string(),
        }
    }
}

impl AuthFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn admin() -> Self {
        Self {
            username: "nacos".to_string(),
            password: "nacos".to_string(),
            role: "ROLE_ADMIN".to_string(),
        }
    }

    pub fn with_username(mut self, username: &str) -> Self {
        self.username = username.to_string();
        self
    }

    pub fn with_password(mut self, password: &str) -> Self {
        self.password = password.to_string();
        self
    }

    pub fn with_role(mut self, role: &str) -> Self {
        self.role = role.to_string();
        self
    }

    pub fn to_login_params(&self) -> Vec<(&str, &str)> {
        vec![("username", &self.username), ("password", &self.password)]
    }

    pub fn to_create_params(&self) -> Vec<(&str, &str)> {
        vec![("username", &self.username), ("password", &self.password)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_fixture_default() {
        let fixture = ConfigFixture::new();
        assert!(fixture.data_id.starts_with("test-config-"));
        assert_eq!(fixture.group, "DEFAULT_GROUP");
        assert_eq!(fixture.config_type, "properties");
    }

    #[test]
    fn test_config_fixture_yaml() {
        let fixture = ConfigFixture::new().yaml();
        assert!(fixture.data_id.ends_with(".yaml"));
        assert_eq!(fixture.config_type, "yaml");
        assert!(fixture.content.contains("server:"));
    }

    #[test]
    fn test_config_fixture_json() {
        let fixture = ConfigFixture::new().json();
        assert!(fixture.data_id.ends_with(".json"));
        assert_eq!(fixture.config_type, "json");
    }

    #[test]
    fn test_config_fixture_builder() {
        let fixture = ConfigFixture::new()
            .with_data_id("custom.yaml")
            .with_group("CUSTOM_GROUP")
            .with_tenant("prod")
            .with_content("custom: true")
            .with_type("yaml");
        assert_eq!(fixture.data_id, "custom.yaml");
        assert_eq!(fixture.group, "CUSTOM_GROUP");
        assert_eq!(fixture.tenant, "prod");
    }

    #[test]
    fn test_config_fixture_to_v2_params() {
        let fixture = ConfigFixture::new()
            .with_data_id("test.yaml")
            .with_content("content");
        let params = fixture.to_v2_publish_params();
        assert!(
            params
                .iter()
                .any(|(k, v)| *k == "dataId" && *v == "test.yaml")
        );
        assert!(
            params
                .iter()
                .any(|(k, v)| *k == "content" && *v == "content")
        );
    }

    #[test]
    fn test_instance_fixture_default() {
        let fixture = InstanceFixture::new();
        assert!(fixture.service_name.starts_with("test-service-"));
        assert_eq!(fixture.ip, "127.0.0.1");
        assert_eq!(fixture.port, 8080);
        assert!(fixture.healthy);
        assert!(fixture.ephemeral);
    }

    #[test]
    fn test_instance_fixture_builder() {
        let fixture = InstanceFixture::new()
            .with_ip("10.0.0.1")
            .with_port(9090)
            .with_weight(2.0)
            .unhealthy()
            .persistent()
            .with_metadata("env", "prod");
        assert_eq!(fixture.ip, "10.0.0.1");
        assert_eq!(fixture.port, 9090);
        assert_eq!(fixture.weight, 2.0);
        assert!(!fixture.healthy);
        assert!(!fixture.ephemeral);
        assert_eq!(fixture.metadata.get("env").unwrap(), "prod");
    }

    #[test]
    fn test_namespace_fixture_default() {
        let fixture = NamespaceFixture::new();
        assert!(fixture.namespace_id.starts_with("test-ns-"));
        assert_eq!(fixture.namespace_name, "Test Namespace");
    }

    #[test]
    fn test_auth_fixture_admin() {
        let fixture = AuthFixture::admin();
        assert_eq!(fixture.username, "nacos");
        assert_eq!(fixture.password, "nacos");
        assert_eq!(fixture.role, "ROLE_ADMIN");
    }

    #[test]
    fn test_auth_fixture_builder() {
        let fixture = AuthFixture::new()
            .with_username("dev")
            .with_password("secret123")
            .with_role("ROLE_DEV");
        assert_eq!(fixture.username, "dev");
        assert_eq!(fixture.role, "ROLE_DEV");
    }
}
