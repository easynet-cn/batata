//! Apollo Open API models
//!
//! Models for Apollo Open API management endpoints.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Apollo App information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloApp {
    /// Application name (used as appId)
    pub name: String,
    /// Application ID
    pub app_id: String,
    /// Organization ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    /// Organization name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_name: Option<String>,
    /// Owner name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_name: Option<String>,
    /// Owner email
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_email: Option<String>,
    /// Data change created by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    /// Data change last modified by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
}

impl ApolloApp {
    /// Create a new ApolloApp
    pub fn new(app_id: String, name: String) -> Self {
        Self {
            name,
            app_id,
            org_id: None,
            org_name: None,
            owner_name: None,
            owner_email: None,
            data_change_created_by: None,
            data_change_last_modified_by: None,
        }
    }
}

/// Apollo Cluster information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloCluster {
    /// Cluster name
    pub name: String,
    /// Application ID
    pub app_id: String,
    /// Data change created by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    /// Data change last modified by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
}

impl ApolloCluster {
    /// Create a new ApolloCluster
    pub fn new(name: String, app_id: String) -> Self {
        Self {
            name,
            app_id,
            data_change_created_by: None,
            data_change_last_modified_by: None,
        }
    }
}

/// Apollo Environment with clusters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloEnvCluster {
    /// Environment name
    pub env: String,
    /// Clusters in this environment
    pub clusters: Vec<String>,
}

impl ApolloEnvCluster {
    /// Create a new ApolloEnvCluster
    pub fn new(env: String, clusters: Vec<String>) -> Self {
        Self { env, clusters }
    }
}

/// Apollo Namespace information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloNamespace {
    /// Application ID
    pub app_id: String,
    /// Cluster name
    pub cluster_name: String,
    /// Namespace name
    pub namespace_name: String,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Format (properties, xml, json, yaml, txt)
    pub format: String,
    /// Is public namespace
    pub is_public: bool,
    /// Items in this namespace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<ApolloItem>>,
    /// Data change created by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    /// Data change last modified by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
}

impl ApolloNamespace {
    /// Create a new ApolloNamespace
    pub fn new(app_id: String, cluster_name: String, namespace_name: String) -> Self {
        let format = if namespace_name.contains('.') {
            namespace_name
                .rsplit('.')
                .next()
                .unwrap_or("properties")
                .to_string()
        } else {
            "properties".to_string()
        };

        Self {
            app_id,
            cluster_name,
            namespace_name,
            comment: None,
            format,
            is_public: false,
            items: None,
            data_change_created_by: None,
            data_change_last_modified_by: None,
        }
    }

    /// Set items
    pub fn with_items(mut self, items: Vec<ApolloItem>) -> Self {
        self.items = Some(items);
        self
    }
}

/// Apollo configuration item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloItem {
    /// Item key
    pub key: String,
    /// Item value
    pub value: String,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Data change created by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    /// Data change last modified by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
    /// Data change created time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
    /// Data change last modified time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_time: Option<String>,
}

impl ApolloItem {
    /// Create a new ApolloItem
    pub fn new(key: String, value: String) -> Self {
        Self {
            key,
            value,
            comment: None,
            data_change_created_by: None,
            data_change_last_modified_by: None,
            data_change_created_time: None,
            data_change_last_modified_time: None,
        }
    }

    /// Set comment
    pub fn with_comment(mut self, comment: String) -> Self {
        self.comment = Some(comment);
        self
    }
}

/// Apollo Release information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloRelease {
    /// Application ID
    pub app_id: String,
    /// Cluster name
    pub cluster_name: String,
    /// Namespace name
    pub namespace_name: String,
    /// Release name
    pub name: String,
    /// Release configurations
    pub configurations: HashMap<String, String>,
    /// Release comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Data change created by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    /// Data change last modified by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
    /// Data change created time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
    /// Data change last modified time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_time: Option<String>,
    /// Is abandoned
    #[serde(default)]
    pub is_abandoned: bool,
}

impl ApolloRelease {
    /// Create a new ApolloRelease
    pub fn new(
        app_id: String,
        cluster_name: String,
        namespace_name: String,
        name: String,
        configurations: HashMap<String, String>,
    ) -> Self {
        Self {
            app_id,
            cluster_name,
            namespace_name,
            name,
            configurations,
            comment: None,
            data_change_created_by: None,
            data_change_last_modified_by: None,
            data_change_created_time: None,
            data_change_last_modified_time: None,
            is_abandoned: false,
        }
    }
}

/// Request to create/update an item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateItemRequest {
    /// Item key
    pub key: String,
    /// Item value
    pub value: String,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Data change created by
    pub data_change_created_by: String,
}

/// Request to update an item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateItemRequest {
    /// Item key
    pub key: String,
    /// Item value
    pub value: String,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Data change last modified by
    pub data_change_last_modified_by: String,
    /// Whether to create if not exists
    #[serde(default)]
    pub create_if_not_exists: bool,
}

/// Request to publish a release
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishReleaseRequest {
    /// Release title
    pub release_title: String,
    /// Release comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_comment: Option<String>,
    /// Released by
    pub released_by: String,
}

/// Request to create an app
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppRequest {
    /// Application name
    pub name: String,
    /// Application ID
    pub app_id: String,
    /// Organization ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    /// Organization name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_name: Option<String>,
    /// Owner name
    pub owner_name: String,
    /// Owner email
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_email: Option<String>,
    /// Admins
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admins: Option<Vec<String>>,
}

/// Request to create a namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNamespaceRequest {
    /// Namespace name
    pub name: String,
    /// Application ID
    pub app_id: String,
    /// Format (properties, xml, json, yaml, txt)
    #[serde(default = "default_format")]
    pub format: String,
    /// Is public namespace
    #[serde(default)]
    pub is_public: bool,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Data change created by
    pub data_change_created_by: String,
}

fn default_format() -> String {
    "properties".to_string()
}

/// Path parameters for Open API endpoints
#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiPathParams {
    /// Environment
    pub env: String,
    /// Application ID
    pub app_id: String,
    /// Cluster name
    pub cluster: String,
    /// Namespace name
    pub namespace: String,
}

/// Path parameters for item endpoints
#[derive(Debug, Clone, Deserialize)]
pub struct ItemPathParams {
    /// Environment
    pub env: String,
    /// Application ID
    pub app_id: String,
    /// Cluster name
    pub cluster: String,
    /// Namespace name
    pub namespace: String,
    /// Item key
    pub key: String,
}

/// Path parameters for app endpoints
#[derive(Debug, Clone, Deserialize)]
pub struct AppPathParams {
    /// Application ID
    pub app_id: String,
}

/// Path parameters for release endpoints
#[derive(Debug, Clone, Deserialize)]
pub struct ReleasePathParams {
    /// Environment
    pub env: String,
    /// Release ID
    pub release_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apollo_app_serialization() {
        let app = ApolloApp::new("test-app".to_string(), "Test Application".to_string());
        let json = serde_json::to_string(&app).unwrap();
        assert!(json.contains("\"appId\":\"test-app\""));
        assert!(json.contains("\"name\":\"Test Application\""));
    }

    #[test]
    fn test_apollo_item_serialization() {
        let item = ApolloItem::new("db.url".to_string(), "jdbc:mysql://localhost".to_string())
            .with_comment("Database URL".to_string());
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"key\":\"db.url\""));
        assert!(json.contains("\"comment\":\"Database URL\""));
    }

    #[test]
    fn test_apollo_namespace_format_detection() {
        let ns1 = ApolloNamespace::new(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
        );
        assert_eq!(ns1.format, "properties");

        let ns2 = ApolloNamespace::new(
            "app1".to_string(),
            "default".to_string(),
            "config.yaml".to_string(),
        );
        assert_eq!(ns2.format, "yaml");

        let ns3 = ApolloNamespace::new(
            "app1".to_string(),
            "default".to_string(),
            "settings.json".to_string(),
        );
        assert_eq!(ns3.format, "json");
    }

    #[test]
    fn test_create_item_request_deserialization() {
        let json = r#"{
            "key": "test.key",
            "value": "test-value",
            "comment": "Test comment",
            "dataChangeCreatedBy": "apollo"
        }"#;
        let req: CreateItemRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.key, "test.key");
        assert_eq!(req.value, "test-value");
        assert_eq!(req.comment, Some("Test comment".to_string()));
        assert_eq!(req.data_change_created_by, "apollo");
    }
}
