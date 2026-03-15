//! Plugin SPI (Service Provider Interface) definitions
//!
//! Standard plugin interfaces for extending Batata functionality.
//! Each plugin type has a trait that implementations must satisfy.

use std::collections::HashMap;
use std::sync::Arc;

/// Plugin lifecycle and metadata
pub trait Plugin: Send + Sync {
    /// Plugin name
    fn name(&self) -> &str;

    /// Plugin type identifier
    fn plugin_type(&self) -> &str;

    /// Plugin priority (lower = higher priority)
    fn priority(&self) -> i32 {
        0
    }

    /// Whether this plugin is enabled
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Authentication plugin for custom auth providers
#[async_trait::async_trait]
pub trait AuthPlugin: Plugin {
    /// Authenticate a user with credentials
    async fn authenticate(
        &self,
        username: &str,
        password: &str,
        properties: &HashMap<String, String>,
    ) -> AuthPluginResult;

    /// Authorize an action on a resource
    async fn authorize(&self, username: &str, resource: &str, action: &str) -> bool;
}

/// Result of authentication
#[derive(Debug, Clone)]
pub struct AuthPluginResult {
    pub success: bool,
    pub username: String,
    pub message: Option<String>,
    pub properties: HashMap<String, String>,
}

/// Config change plugin for intercepting config operations
#[async_trait::async_trait]
pub trait ConfigChangePlugin: Plugin {
    /// Called before a config is published/updated
    /// Return false to abort the operation
    async fn before_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
    ) -> ConfigChangeResult;

    /// Called after a config is published/updated
    async fn after_publish(&self, data_id: &str, group: &str, tenant: &str, content: &str);

    /// Called before a config is deleted
    async fn before_delete(&self, data_id: &str, group: &str, tenant: &str) -> ConfigChangeResult;

    /// Called after a config is deleted
    async fn after_delete(&self, data_id: &str, group: &str, tenant: &str);
}

/// Result of config change interception
#[derive(Debug, Clone)]
pub struct ConfigChangeResult {
    pub allowed: bool,
    pub message: Option<String>,
}

impl ConfigChangeResult {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            message: None,
        }
    }

    pub fn deny(message: &str) -> Self {
        Self {
            allowed: false,
            message: Some(message.to_string()),
        }
    }
}

/// Traffic control plugin for rate limiting
#[async_trait::async_trait]
pub trait ControlPlugin: Plugin {
    /// Check if a request should be allowed based on TPS rules
    async fn check_tps(&self, point_name: &str, client_ip: &str) -> TpsCheckResult;
}

/// Result of TPS check
#[derive(Debug, Clone)]
pub struct TpsCheckResult {
    pub allowed: bool,
    pub remaining: u64,
    pub limit: u64,
}

/// Plugin manager for registering and invoking plugins
pub struct PluginManager {
    auth_plugins: Vec<Arc<dyn AuthPlugin>>,
    config_change_plugins: Vec<Arc<dyn ConfigChangePlugin>>,
    control_plugins: Vec<Arc<dyn ControlPlugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            auth_plugins: Vec::new(),
            config_change_plugins: Vec::new(),
            control_plugins: Vec::new(),
        }
    }

    pub fn register_auth(&mut self, plugin: Arc<dyn AuthPlugin>) {
        self.auth_plugins.push(plugin);
        self.auth_plugins.sort_by_key(|p| p.priority());
    }

    pub fn register_config_change(&mut self, plugin: Arc<dyn ConfigChangePlugin>) {
        self.config_change_plugins.push(plugin);
        self.config_change_plugins.sort_by_key(|p| p.priority());
    }

    pub fn register_control(&mut self, plugin: Arc<dyn ControlPlugin>) {
        self.control_plugins.push(plugin);
        self.control_plugins.sort_by_key(|p| p.priority());
    }

    /// Execute all config change plugins before publish
    pub async fn before_config_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
    ) -> ConfigChangeResult {
        for plugin in &self.config_change_plugins {
            if !plugin.is_enabled() {
                continue;
            }
            let result = plugin.before_publish(data_id, group, tenant, content).await;
            if !result.allowed {
                return result;
            }
        }
        ConfigChangeResult::allow()
    }

    /// Execute all config change plugins after publish
    pub async fn after_config_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
    ) {
        for plugin in &self.config_change_plugins {
            if plugin.is_enabled() {
                plugin.after_publish(data_id, group, tenant, content).await;
            }
        }
    }

    /// Check TPS across all control plugins
    pub async fn check_tps(&self, point_name: &str, client_ip: &str) -> TpsCheckResult {
        for plugin in &self.control_plugins {
            if !plugin.is_enabled() {
                continue;
            }
            let result = plugin.check_tps(point_name, client_ip).await;
            if !result.allowed {
                return result;
            }
        }
        TpsCheckResult {
            allowed: true,
            remaining: u64::MAX,
            limit: u64::MAX,
        }
    }

    pub fn auth_plugins(&self) -> &[Arc<dyn AuthPlugin>] {
        &self.auth_plugins
    }

    pub fn config_change_plugins(&self) -> &[Arc<dyn ConfigChangePlugin>] {
        &self.config_change_plugins
    }

    pub fn control_plugins(&self) -> &[Arc<dyn ControlPlugin>] {
        &self.control_plugins
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAuthPlugin;

    impl Plugin for TestAuthPlugin {
        fn name(&self) -> &str {
            "test-auth"
        }
        fn plugin_type(&self) -> &str {
            "auth"
        }
        fn priority(&self) -> i32 {
            10
        }
    }

    #[async_trait::async_trait]
    impl AuthPlugin for TestAuthPlugin {
        async fn authenticate(
            &self,
            username: &str,
            _password: &str,
            _props: &HashMap<String, String>,
        ) -> AuthPluginResult {
            AuthPluginResult {
                success: username == "admin",
                username: username.to_string(),
                message: None,
                properties: HashMap::new(),
            }
        }
        async fn authorize(&self, _username: &str, _resource: &str, _action: &str) -> bool {
            true
        }
    }

    struct TestConfigPlugin;

    impl Plugin for TestConfigPlugin {
        fn name(&self) -> &str {
            "test-config"
        }
        fn plugin_type(&self) -> &str {
            "config-change"
        }
    }

    #[async_trait::async_trait]
    impl ConfigChangePlugin for TestConfigPlugin {
        async fn before_publish(
            &self,
            _data_id: &str,
            _group: &str,
            _tenant: &str,
            content: &str,
        ) -> ConfigChangeResult {
            if content.contains("FORBIDDEN") {
                ConfigChangeResult::deny("Content contains forbidden keyword")
            } else {
                ConfigChangeResult::allow()
            }
        }
        async fn after_publish(&self, _data_id: &str, _group: &str, _tenant: &str, _content: &str) {
        }
        async fn before_delete(
            &self,
            _data_id: &str,
            _group: &str,
            _tenant: &str,
        ) -> ConfigChangeResult {
            ConfigChangeResult::allow()
        }
        async fn after_delete(&self, _data_id: &str, _group: &str, _tenant: &str) {}
    }

    #[tokio::test]
    async fn test_plugin_manager_config_change() {
        let mut manager = PluginManager::new();
        manager.register_config_change(Arc::new(TestConfigPlugin));

        let result = manager
            .before_config_publish("app.yaml", "DEFAULT_GROUP", "public", "key: value")
            .await;
        assert!(result.allowed);

        let result = manager
            .before_config_publish("app.yaml", "DEFAULT_GROUP", "public", "FORBIDDEN content")
            .await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_plugin_manager_auth() {
        let mut manager = PluginManager::new();
        manager.register_auth(Arc::new(TestAuthPlugin));

        let result = manager.auth_plugins()[0]
            .authenticate("admin", "pass", &HashMap::new())
            .await;
        assert!(result.success);

        let result = manager.auth_plugins()[0]
            .authenticate("user", "pass", &HashMap::new())
            .await;
        assert!(!result.success);
    }

    #[test]
    fn test_config_change_result() {
        let allow = ConfigChangeResult::allow();
        assert!(allow.allowed);
        assert!(allow.message.is_none());

        let deny = ConfigChangeResult::deny("not allowed");
        assert!(!deny.allowed);
        assert_eq!(deny.message.as_deref(), Some("not allowed"));
    }

    #[test]
    fn test_plugin_priority_ordering() {
        let mut manager = PluginManager::new();

        struct LowPriority;
        impl Plugin for LowPriority {
            fn name(&self) -> &str {
                "low"
            }
            fn plugin_type(&self) -> &str {
                "auth"
            }
            fn priority(&self) -> i32 {
                100
            }
        }
        #[async_trait::async_trait]
        impl AuthPlugin for LowPriority {
            async fn authenticate(
                &self,
                _: &str,
                _: &str,
                _: &HashMap<String, String>,
            ) -> AuthPluginResult {
                AuthPluginResult {
                    success: true,
                    username: String::new(),
                    message: None,
                    properties: HashMap::new(),
                }
            }
            async fn authorize(&self, _: &str, _: &str, _: &str) -> bool {
                true
            }
        }

        struct HighPriority;
        impl Plugin for HighPriority {
            fn name(&self) -> &str {
                "high"
            }
            fn plugin_type(&self) -> &str {
                "auth"
            }
            fn priority(&self) -> i32 {
                1
            }
        }
        #[async_trait::async_trait]
        impl AuthPlugin for HighPriority {
            async fn authenticate(
                &self,
                _: &str,
                _: &str,
                _: &HashMap<String, String>,
            ) -> AuthPluginResult {
                AuthPluginResult {
                    success: true,
                    username: String::new(),
                    message: None,
                    properties: HashMap::new(),
                }
            }
            async fn authorize(&self, _: &str, _: &str, _: &str) -> bool {
                true
            }
        }

        // Register in reverse order
        manager.register_auth(Arc::new(LowPriority));
        manager.register_auth(Arc::new(HighPriority));

        // High priority should be first
        assert_eq!(manager.auth_plugins()[0].name(), "high");
        assert_eq!(manager.auth_plugins()[1].name(), "low");
    }
}
