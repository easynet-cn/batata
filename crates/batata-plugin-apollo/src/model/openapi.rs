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
    fn test_apollo_app_optional_fields_skipped() {
        let app = ApolloApp::new("test-app".to_string(), "Test App".to_string());
        let json = serde_json::to_string(&app).unwrap();
        // Optional None fields should be skipped
        assert!(!json.contains("orgId"));
        assert!(!json.contains("ownerName"));
    }

    #[test]
    fn test_apollo_app_deserialization() {
        let json = r#"{
            "name": "My App",
            "appId": "my-app",
            "orgId": "org1",
            "orgName": "Engineering",
            "ownerName": "admin",
            "ownerEmail": "admin@example.com"
        }"#;
        let app: ApolloApp = serde_json::from_str(json).unwrap();
        assert_eq!(app.app_id, "my-app");
        assert_eq!(app.name, "My App");
        assert_eq!(app.org_id, Some("org1".to_string()));
        assert_eq!(app.owner_name, Some("admin".to_string()));
    }

    #[test]
    fn test_apollo_cluster_serialization() {
        let cluster = ApolloCluster::new("default".to_string(), "app1".to_string());
        let json = serde_json::to_string(&cluster).unwrap();
        assert!(json.contains("\"name\":\"default\""));
        assert!(json.contains("\"appId\":\"app1\""));
        // Optional fields skipped
        assert!(!json.contains("dataChangeCreatedBy"));
    }

    #[test]
    fn test_apollo_env_cluster_serialization() {
        let ec = ApolloEnvCluster::new(
            "DEV".to_string(),
            vec!["default".to_string(), "staging".to_string()],
        );
        let json = serde_json::to_string(&ec).unwrap();
        assert!(json.contains("\"env\":\"DEV\""));
        assert!(json.contains("\"clusters\":[\"default\",\"staging\"]"));
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
    fn test_apollo_item_without_comment() {
        let item = ApolloItem::new("key".to_string(), "value".to_string());
        let json = serde_json::to_string(&item).unwrap();
        assert!(!json.contains("comment"));
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
    fn test_apollo_namespace_format_xml_and_txt() {
        let ns_xml = ApolloNamespace::new(
            "app1".to_string(),
            "default".to_string(),
            "config.xml".to_string(),
        );
        assert_eq!(ns_xml.format, "xml");

        let ns_txt = ApolloNamespace::new(
            "app1".to_string(),
            "default".to_string(),
            "readme.txt".to_string(),
        );
        assert_eq!(ns_txt.format, "txt");

        let ns_yml = ApolloNamespace::new(
            "app1".to_string(),
            "default".to_string(),
            "app.yml".to_string(),
        );
        assert_eq!(ns_yml.format, "yml");
    }

    #[test]
    fn test_apollo_namespace_with_items() {
        let items = vec![
            ApolloItem::new("key1".to_string(), "val1".to_string()),
            ApolloItem::new("key2".to_string(), "val2".to_string()),
        ];
        let ns = ApolloNamespace::new(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
        )
        .with_items(items);
        assert_eq!(ns.items.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_apollo_release_serialization() {
        let mut configs = HashMap::new();
        configs.insert("db.url".to_string(), "jdbc:mysql://localhost".to_string());
        configs.insert("db.pool".to_string(), "10".to_string());

        let release = ApolloRelease::new(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
            "Release-20240101".to_string(),
            configs,
        );

        let json = serde_json::to_string(&release).unwrap();
        assert!(json.contains("\"appId\":\"app1\""));
        assert!(json.contains("\"name\":\"Release-20240101\""));
        assert!(!release.is_abandoned);
    }

    #[test]
    fn test_apollo_release_deserialization() {
        let json = r#"{
            "appId": "app1",
            "clusterName": "default",
            "namespaceName": "application",
            "name": "v1.0",
            "configurations": {"key1": "val1"},
            "isAbandoned": true
        }"#;
        let release: ApolloRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.app_id, "app1");
        assert!(release.is_abandoned);
        assert_eq!(
            release.configurations.get("key1"),
            Some(&"val1".to_string())
        );
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

    #[test]
    fn test_create_app_request_deserialization() {
        let json = r#"{
            "name": "My App",
            "appId": "my-app",
            "ownerName": "admin",
            "orgId": "eng",
            "orgName": "Engineering"
        }"#;
        let req: CreateAppRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.app_id, "my-app");
        assert_eq!(req.name, "My App");
        assert_eq!(req.owner_name, "admin");
        assert_eq!(req.org_id, Some("eng".to_string()));
        assert!(req.admins.is_none());
    }

    #[test]
    fn test_create_app_request_with_admins() {
        let json = r#"{
            "name": "App",
            "appId": "app1",
            "ownerName": "owner",
            "admins": ["admin1", "admin2"]
        }"#;
        let req: CreateAppRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.admins.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_create_namespace_request_default_format() {
        let json = r#"{
            "name": "application",
            "appId": "app1",
            "dataChangeCreatedBy": "admin"
        }"#;
        let req: CreateNamespaceRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.format, "properties");
        assert!(!req.is_public);
    }

    #[test]
    fn test_create_namespace_request_custom_format() {
        let json = r#"{
            "name": "config.yaml",
            "appId": "app1",
            "format": "yaml",
            "isPublic": true,
            "comment": "YAML config",
            "dataChangeCreatedBy": "admin"
        }"#;
        let req: CreateNamespaceRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.format, "yaml");
        assert!(req.is_public);
        assert_eq!(req.comment, Some("YAML config".to_string()));
    }

    #[test]
    fn test_update_item_request_deserialization() {
        let json = r#"{
            "key": "db.url",
            "value": "jdbc:mysql://newhost",
            "dataChangeLastModifiedBy": "admin",
            "createIfNotExists": true
        }"#;
        let req: UpdateItemRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.key, "db.url");
        assert!(req.create_if_not_exists);
    }

    #[test]
    fn test_update_item_request_default_create_if_not_exists() {
        let json = r#"{
            "key": "k",
            "value": "v",
            "dataChangeLastModifiedBy": "admin"
        }"#;
        let req: UpdateItemRequest = serde_json::from_str(json).unwrap();
        assert!(!req.create_if_not_exists);
    }

    #[test]
    fn test_publish_release_request() {
        let json = r#"{
            "releaseTitle": "v1.0",
            "releaseComment": "First release",
            "releasedBy": "admin"
        }"#;
        let req: PublishReleaseRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.release_title, "v1.0");
        assert_eq!(req.release_comment, Some("First release".to_string()));
        assert_eq!(req.released_by, "admin");
    }

    #[test]
    fn test_open_api_path_params() {
        let json = r#"{"env":"DEV","app_id":"app1","cluster":"default","namespace":"application"}"#;
        let params: OpenApiPathParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.env, "DEV");
        assert_eq!(params.namespace, "application");
    }

    #[test]
    fn test_item_path_params() {
        let json = r#"{"env":"DEV","app_id":"app1","cluster":"default","namespace":"application","key":"db.url"}"#;
        let params: ItemPathParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.key, "db.url");
    }

    #[test]
    fn test_app_path_params() {
        let json = r#"{"app_id":"my-app"}"#;
        let params: AppPathParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.app_id, "my-app");
    }

    #[test]
    fn test_release_path_params() {
        let json = r#"{"env":"PRO","release_id":"12345"}"#;
        let params: ReleasePathParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.release_id, "12345");
    }
}
