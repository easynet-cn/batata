//! Configuration service traits: ConsoleDataSource, ConfigSubscriptionService,
//! ConfigEncryptionProvider and related types.

use crate::crypto::CryptoResult;

/// Console data source trait
///
/// Abstracts console operations that can be performed either
/// locally (direct database access) or remotely (via HTTP to leader).
#[async_trait::async_trait]
pub trait ConsoleDataSource: Send + Sync {
    /// Check if this is a remote data source
    fn is_remote(&self) -> bool;

    // Namespace operations

    /// Find all namespaces
    async fn namespace_find_all(&self) -> Vec<NamespaceInfo>;

    /// Get namespace by ID
    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
        tenant_id: &str,
    ) -> anyhow::Result<NamespaceInfo>;

    /// Create a new namespace
    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<()>;

    /// Update a namespace
    async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool>;

    /// Delete a namespace
    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool>;

    /// Check if a namespace exists
    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool>;
}

/// Namespace information
#[derive(Debug, Clone, Default)]
pub struct NamespaceInfo {
    pub namespace_id: String,
    pub namespace_name: String,
    pub namespace_desc: String,
    pub quota: i32,
    pub config_count: i32,
    pub namespace_type: i32,
}

/// Connection metadata for trait-based access
#[derive(Debug, Clone, Default)]
pub struct ConnectionInfo {
    pub connection_id: String,
    pub client_ip: String,
    pub client_port: u16,
    pub app_name: String,
    pub sdk: String,
    pub version: String,
    pub labels: std::collections::HashMap<String, String>,
    pub create_time: u64,
    pub last_active_time: u64,
}

/// Key for a configuration subscription
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConfigSubscriptionKey {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
}

impl ConfigSubscriptionKey {
    pub fn new(data_id: &str, group: &str, tenant: &str) -> Self {
        Self {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
        }
    }
}

/// Information about a config subscriber
#[derive(Clone, Debug)]
pub struct ConfigSubscriberInfo {
    pub connection_id: String,
    pub client_ip: String,
    pub md5: String,
    pub client_tenant: String,
}

/// Config subscription service trait
///
/// Abstracts config subscription tracking operations.
/// This allows handlers and console to manage subscriptions
/// without depending on the concrete `ConfigSubscriberManager` type.
pub trait ConfigSubscriptionService: Send + Sync {
    /// Register a subscription for a configuration
    fn subscribe(
        &self,
        connection_id: &str,
        client_ip: &str,
        key: &ConfigSubscriptionKey,
        md5: &str,
        client_tenant: &str,
    );

    /// Unsubscribe from a specific configuration
    fn unsubscribe(&self, connection_id: &str, key: &ConfigSubscriptionKey);

    /// Unsubscribe from all configurations for a connection
    fn unsubscribe_all(&self, connection_id: &str);

    /// Get all subscribers for a specific configuration
    fn get_subscribers(&self, key: &ConfigSubscriptionKey) -> Vec<ConfigSubscriberInfo>;

    /// Get all subscribers by client IP
    fn get_subscribers_by_ip(
        &self,
        client_ip: &str,
    ) -> Vec<(ConfigSubscriptionKey, ConfigSubscriberInfo)>;

    /// Get all subscriptions
    fn get_all_subscriptions(&self) -> Vec<(ConfigSubscriptionKey, Vec<ConfigSubscriberInfo>)>;

    /// Update the MD5 for a subscriber
    fn update_md5(&self, connection_id: &str, key: &ConfigSubscriptionKey, md5: &str);

    /// Get total number of subscriptions
    fn subscription_count(&self) -> usize;

    /// Get number of unique configs being watched
    fn config_count(&self) -> usize;

    /// Get number of connections with active subscriptions
    fn subscriber_connection_count(&self) -> usize;
}

/// Configuration encryption provider trait
///
/// Abstracts config encryption/decryption operations,
/// allowing AppState to hold a trait object instead of `Arc<dyn Any>`.
#[async_trait::async_trait]
pub trait ConfigEncryptionProvider: Send + Sync {
    /// Check if encryption is enabled
    fn is_enabled(&self) -> bool;

    /// Check if a data_id should be encrypted based on configured patterns
    fn should_encrypt(&self, data_id: &str) -> bool;

    /// Encrypt content if the data_id matches encryption patterns
    ///
    /// Returns (possibly encrypted content, encrypted_data_key).
    /// If encryption is disabled or data_id doesn't match, returns original content
    /// with an empty data key.
    async fn encrypt_if_needed(&self, data_id: &str, content: &str) -> (String, String);

    /// Decrypt content if it has an encrypted data key
    ///
    /// If the encrypted_data_key is empty, returns the content as-is.
    async fn decrypt_if_needed(
        &self,
        data_id: &str,
        content: &str,
        encrypted_data_key: &str,
    ) -> String;

    /// Encrypt content directly
    async fn encrypt(&self, content: &str) -> CryptoResult<(String, String)>;

    /// Decrypt content directly
    async fn decrypt(&self, content: &str, encrypted_data_key: &str) -> CryptoResult<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_info_default() {
        let ns = NamespaceInfo::default();
        assert!(ns.namespace_id.is_empty());
        assert!(ns.namespace_name.is_empty());
        assert_eq!(ns.config_count, 0);
        assert_eq!(ns.quota, 0);
    }

    #[test]
    fn test_config_subscription_key() {
        let key = ConfigSubscriptionKey::new("app.yaml", "DEFAULT_GROUP", "public");
        assert_eq!(key.data_id, "app.yaml");
        assert_eq!(key.group, "DEFAULT_GROUP");
        assert_eq!(key.tenant, "public");
    }

    #[test]
    fn test_connection_info_default() {
        let info = ConnectionInfo::default();
        assert!(info.connection_id.is_empty());
        assert!(info.client_ip.is_empty());
        assert_eq!(info.create_time, 0);
    }
}
