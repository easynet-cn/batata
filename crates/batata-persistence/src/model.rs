//! Domain model types for the persistence abstraction layer
//!
//! These types are used as return values from the persistence traits,
//! decoupled from specific storage backends.

use serde::{Deserialize, Serialize};

/// Basic user information returned from persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Username (unique login identifier).
    pub username: String,
    /// Hashed password. Empty/sentinel for externally sourced users.
    pub password: String,
    /// Whether the account is enabled.
    pub enabled: bool,
    /// Identity source: "local" (default), "oauth", or "ldap".
    #[serde(default = "default_user_source")]
    pub source: String,
}

fn default_user_source() -> String {
    "local".to_string()
}

/// Role assignment information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    /// Role name (e.g. `ROLE_ADMIN`).
    pub role: String,
    /// Username this role assignment belongs to.
    pub username: String,
}

/// Permission information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionInfo {
    /// Role this permission is granted to.
    pub role: String,
    /// Resource pattern the permission applies to.
    pub resource: String,
    /// Allowed action on the resource (e.g. `r`, `w`).
    pub action: String,
}

// ============================================================================
// AI Resource persistence models
// ============================================================================

/// AI resource metadata (storage-agnostic representation of ai_resource row)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResourceInfo {
    /// Primary key.
    pub id: i64,
    /// Resource name.
    pub name: String,
    /// Resource type (e.g. `skill`, `agentspec`).
    pub resource_type: String,
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Optional lifecycle status.
    pub status: Option<String>,
    /// Owning namespace ID.
    pub namespace_id: String,
    /// Optional comma-separated business tags.
    pub biz_tags: Option<String>,
    /// Optional opaque extension JSON.
    pub ext: Option<String>,
    /// Origin of the resource (e.g. `user`, `system`).
    pub from: String,
    /// Optional serialized version metadata.
    pub version_info: Option<String>,
    /// Optimistic-lock version for `version_info` updates.
    pub meta_version: i64,
    /// Visibility scope (e.g. `PUBLIC`, `PRIVATE`).
    pub scope: String,
    /// Owner username.
    pub owner: String,
    /// Number of downloads.
    pub download_count: i64,
    /// Creation timestamp (string form).
    pub gmt_create: Option<String>,
    /// Last modification timestamp (string form).
    pub gmt_modified: Option<String>,
}

/// AI resource version (storage-agnostic representation of ai_resource_version row)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResourceVersionInfo {
    /// Primary key.
    pub id: i64,
    /// Resource type (e.g. `skill`, `agentspec`).
    pub resource_type: String,
    /// Optional author username.
    pub author: Option<String>,
    /// Resource name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Lifecycle status.
    pub status: String,
    /// Version string.
    pub version: String,
    /// Owning namespace ID.
    pub namespace_id: String,
    /// Optional serialized storage location.
    pub storage: Option<String>,
    /// Optional serialized publish pipeline metadata.
    pub publish_pipeline_info: Option<String>,
    /// Number of downloads.
    pub download_count: i64,
    /// Creation timestamp (string form).
    pub gmt_create: Option<String>,
    /// Last modification timestamp (string form).
    pub gmt_modified: Option<String>,
}

/// Pipeline execution (storage-agnostic representation of pipeline_execution row)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineExecutionInfo {
    /// Unique execution ID.
    pub execution_id: String,
    /// Resource type the pipeline ran for.
    pub resource_type: String,
    /// Resource name the pipeline ran for.
    pub resource_name: String,
    /// Optional owning namespace ID.
    pub namespace_id: Option<String>,
    /// Optional resource version.
    pub version: Option<String>,
    /// Execution status.
    pub status: String,
    /// Serialized pipeline definition.
    pub pipeline: String,
    /// Creation time (epoch millis).
    pub create_time: i64,
    /// Last update time (epoch millis).
    pub update_time: i64,
}

/// Filter parameters for ai_resource list queries.
///
/// Used by `AiResourcePersistence::ai_resource_list` to apply
/// visibility-aware filtering (scope, owner) alongside name search.
#[derive(Debug, Clone, Default)]
pub struct AiResourceListFilter<'a> {
    /// Optional name substring (or exact) match.
    pub name_filter: Option<&'a str>,
    /// When true, `name_filter` matches exactly instead of as a substring.
    pub search_accurate: bool,
    /// When true, order results by download count descending.
    pub order_by_downloads: bool,
    /// Filter by exact scope value (e.g., "PUBLIC", "PRIVATE")
    pub scope_filter: Option<&'a str>,
    /// Filter by exact owner value
    pub owner_filter: Option<&'a str>,
    /// When true alongside owner_filter, also include PUBLIC resources
    pub include_public_for_owner: bool,
}

impl<'a> AiResourceListFilter<'a> {
    /// Create an empty filter with default (no-op) settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the name filter and whether matching is exact.
    pub fn with_name_filter(mut self, name: Option<&'a str>, accurate: bool) -> Self {
        self.name_filter = name;
        self.search_accurate = accurate;
        self
    }

    /// Enable or disable ordering results by download count.
    pub fn with_order_by_downloads(mut self, enabled: bool) -> Self {
        self.order_by_downloads = enabled;
        self
    }

    /// Set the exact scope filter.
    pub fn with_scope(mut self, scope: Option<&'a str>) -> Self {
        self.scope_filter = scope;
        self
    }

    /// Set the exact owner filter and whether PUBLIC resources are included.
    pub fn with_owner(mut self, owner: Option<&'a str>, include_public: bool) -> Self {
        self.owner_filter = owner;
        self.include_public_for_owner = include_public;
        self
    }
}

/// Generic paginated result (re-exported from batata-common)
pub use batata_common::model::Page;

/// Storage backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageBackend {
    /// External database (MySQL/PostgreSQL via SeaORM)
    ExternalDb,
    /// Embedded RocksDB (no external DB required)
    Embedded,
}

impl std::fmt::Display for StorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageBackend::ExternalDb => write!(f, "external_db"),
            StorageBackend::Embedded => write!(f, "embedded"),
        }
    }
}

/// Deployment topology
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployTopology {
    /// Single node
    Standalone,
    /// Raft consensus cluster
    Cluster,
}

impl std::fmt::Display for DeployTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployTopology::Standalone => write!(f, "standalone"),
            DeployTopology::Cluster => write!(f, "cluster"),
        }
    }
}

/// Storage mode for the persistence layer (derived from StorageBackend + DeployTopology)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageMode {
    /// External database (MySQL/PostgreSQL via SeaORM)
    ExternalDb,
    /// Standalone embedded RocksDB (single node, no external DB)
    StandaloneEmbedded,
    /// Distributed embedded RocksDB (Raft cluster, no external DB)
    DistributedEmbedded,
}

impl StorageMode {
    /// Derive StorageMode from the two independent dimensions
    pub fn from_dimensions(backend: StorageBackend, topology: DeployTopology) -> Self {
        match (backend, topology) {
            (StorageBackend::ExternalDb, _) => StorageMode::ExternalDb,
            (StorageBackend::Embedded, DeployTopology::Standalone) => {
                StorageMode::StandaloneEmbedded
            }
            (StorageBackend::Embedded, DeployTopology::Cluster) => StorageMode::DistributedEmbedded,
        }
    }

    /// Return the storage backend of this mode.
    pub fn backend(&self) -> StorageBackend {
        match self {
            StorageMode::ExternalDb => StorageBackend::ExternalDb,
            StorageMode::StandaloneEmbedded | StorageMode::DistributedEmbedded => {
                StorageBackend::Embedded
            }
        }
    }

    /// Return the deploy topology of this mode.
    pub fn topology(&self) -> DeployTopology {
        match self {
            StorageMode::ExternalDb | StorageMode::StandaloneEmbedded => DeployTopology::Standalone,
            StorageMode::DistributedEmbedded => DeployTopology::Cluster,
        }
    }
}

impl std::fmt::Display for StorageMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageMode::ExternalDb => write!(f, "external_db"),
            StorageMode::StandaloneEmbedded => write!(f, "standalone_embedded"),
            StorageMode::DistributedEmbedded => write!(f, "distributed_embedded"),
        }
    }
}

impl std::str::FromStr for StorageMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "external_db" => Ok(StorageMode::ExternalDb),
            "standalone_embedded" => Ok(StorageMode::StandaloneEmbedded),
            "distributed_embedded" => Ok(StorageMode::DistributedEmbedded),
            _ => Err(format!("Invalid storage mode: {}", s)),
        }
    }
}

/// Namespace information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceInfo {
    /// Unique namespace identifier.
    pub namespace_id: String,
    /// Human-readable namespace name.
    pub namespace_name: String,
    /// Namespace description.
    pub namespace_desc: String,
    /// Number of configs contained in the namespace.
    pub config_count: i64,
    /// Config quota for the namespace.
    pub quota: i32,
}

/// Config information stored in embedded backends
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfigStorageData {
    /// Primary key.
    pub id: i64,
    /// Config data ID.
    pub data_id: String,
    /// Config group.
    pub group: String,
    /// Tenant ID (namespace).
    pub tenant: String,
    /// Config content.
    pub content: String,
    /// MD5 hash of the content.
    pub md5: String,
    /// Owning application name.
    pub app_name: String,
    /// Config type (e.g. `properties`, `yaml`).
    pub config_type: String,
    /// Description.
    pub desc: String,
    /// Usage notes.
    pub r#use: String,
    /// Effect description.
    pub effect: String,
    /// Schema.
    pub schema: String,
    /// Comma-separated config tags.
    pub config_tags: String,
    /// Encrypted data key (for encrypted content).
    pub encrypted_data_key: String,
    /// User who created/modified the config.
    pub src_user: String,
    /// Source IP of the last modification.
    pub src_ip: String,
    /// Creation time (epoch millis).
    pub created_time: i64,
    /// Modification time (epoch millis).
    pub modified_time: i64,
}

/// Config history entry stored in embedded backends
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfigHistoryStorageData {
    /// Primary key of the history entry.
    pub id: i64,
    /// Config data ID.
    pub data_id: String,
    /// Config group.
    pub group: String,
    /// Tenant ID (namespace).
    pub tenant: String,
    /// Config content at this historical version.
    pub content: String,
    /// MD5 hash of the content.
    pub md5: String,
    /// Owning application name.
    pub app_name: String,
    /// User who performed the operation.
    pub src_user: String,
    /// Source IP of the operation.
    pub src_ip: String,
    /// Operation type (`I`, `U`, `D`).
    pub op_type: String,
    /// Publish type (e.g. `formal`).
    pub publish_type: String,
    /// Gray (beta) config name, if applicable.
    pub gray_name: String,
    /// Serialized extension info (tags, desc, etc.).
    pub ext_info: String,
    /// Encrypted data key.
    pub encrypted_data_key: String,
    /// Creation time (epoch millis).
    pub created_time: i64,
    /// Modification time (epoch millis).
    pub modified_time: i64,
}

/// Capacity information for tenant or group quotas
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CapacityInfo {
    /// Unique identifier
    pub id: Option<i64>,
    /// Tenant ID or Group ID
    pub identifier: String,
    /// Maximum number of configs allowed
    pub quota: i32,
    /// Current usage count
    pub usage: i32,
    /// Maximum config size in bytes
    pub max_size: i32,
    /// Maximum aggregate config count
    pub max_aggr_count: i32,
    /// Maximum aggregate config size
    pub max_aggr_size: i32,
    /// Maximum history count
    pub max_history_count: i32,
}

/// Gray config data stored in embedded backends
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfigGrayStorageData {
    /// Config data ID.
    pub data_id: String,
    /// Config group.
    pub group: String,
    /// Tenant ID (namespace).
    pub tenant: String,
    /// Gray config content.
    pub content: String,
    /// MD5 hash of the content.
    pub md5: String,
    /// Owning application name.
    pub app_name: String,
    /// Gray (beta) config name.
    pub gray_name: String,
    /// Gray rule expression.
    pub gray_rule: String,
    /// Encrypted data key.
    pub encrypted_data_key: String,
    /// User who created/modified the config.
    pub src_user: String,
    /// Source IP of the last modification.
    pub src_ip: String,
    /// Creation time (epoch millis).
    pub created_time: i64,
    /// Modification time (epoch millis).
    pub modified_time: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_mode_display() {
        assert_eq!(StorageMode::ExternalDb.to_string(), "external_db");
        assert_eq!(
            StorageMode::StandaloneEmbedded.to_string(),
            "standalone_embedded"
        );
        assert_eq!(
            StorageMode::DistributedEmbedded.to_string(),
            "distributed_embedded"
        );
    }

    #[test]
    fn test_storage_mode_from_str() {
        assert_eq!(
            "external_db".parse::<StorageMode>().unwrap(),
            StorageMode::ExternalDb
        );
        assert_eq!(
            "standalone_embedded".parse::<StorageMode>().unwrap(),
            StorageMode::StandaloneEmbedded
        );
        assert_eq!(
            "distributed_embedded".parse::<StorageMode>().unwrap(),
            StorageMode::DistributedEmbedded
        );
        assert!("invalid".parse::<StorageMode>().is_err());
    }

    #[test]
    fn test_page_new() {
        let page = Page::<String>::new(100, 1, 10, vec!["a".to_string()]);
        assert_eq!(page.total_count, 100);
        assert_eq!(page.page_number, 1);
        assert_eq!(page.pages_available, 10);
        assert_eq!(page.page_items.len(), 1);
    }

    #[test]
    fn test_page_empty() {
        let page = Page::<String>::empty();
        assert_eq!(page.total_count, 0);
        assert!(page.page_items.is_empty());
    }

    #[test]
    fn test_page_with_items() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let page = Page::new(30, 2, 10, items);
        assert_eq!(page.total_count, 30);
        assert_eq!(page.page_number, 2);
        assert_eq!(page.pages_available, 3);
        assert_eq!(page.page_items.len(), 3);
    }

    #[test]
    fn test_page_rounding_up() {
        // 11 items with page size 5 = 3 pages (ceil(11/5))
        let page = Page::<String>::new(11, 1, 5, vec![]);
        assert_eq!(page.pages_available, 3);
    }

    #[test]
    fn test_page_single_page() {
        let page = Page::<String>::new(3, 1, 10, vec![]);
        assert_eq!(page.pages_available, 1);
    }

    #[test]
    fn test_page_large_dataset() {
        let page = Page::<String>::new(1_000_000, 100, 100, vec![]);
        assert_eq!(page.pages_available, 10_000);
    }

    #[test]
    fn test_page_serialization() {
        let page = Page::new(10, 1, 5, vec!["item1".to_string()]);
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"totalCount\":10"));
        assert!(json.contains("\"pageNumber\":1"));
        assert!(json.contains("\"pagesAvailable\":2"));
        assert!(json.contains("\"pageItems\":[\"item1\"]"));
    }

    #[test]
    fn test_page_deserialization() {
        let json = r#"{"totalCount":5,"pageNumber":1,"pagesAvailable":1,"pageItems":["a","b"]}"#;
        let page: Page<String> = serde_json::from_str(json).unwrap();
        assert_eq!(page.total_count, 5);
        assert_eq!(page.page_items.len(), 2);
    }

    #[test]
    fn test_storage_mode_invalid_from_str() {
        assert!("mysql".parse::<StorageMode>().is_err());
        assert!("ExternalDb".parse::<StorageMode>().is_err());
        assert!("".parse::<StorageMode>().is_err());
    }

    #[test]
    fn test_user_info_creation() {
        let user = UserInfo {
            username: "admin".to_string(),
            password: "hashed".to_string(),
            enabled: true,
            source: "local".to_string(),
        };
        assert_eq!(user.username, "admin");
        assert!(user.enabled);
    }

    #[test]
    fn test_user_info_serialization() {
        let user = UserInfo {
            username: "test".to_string(),
            password: "pass".to_string(),
            enabled: false,
            source: "local".to_string(),
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("\"username\":\"test\""));
        assert!(json.contains("\"enabled\":false"));

        let deserialized: UserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.username, "test");
        assert!(!deserialized.enabled);
    }

    #[test]
    fn test_role_info_serialization() {
        let role = RoleInfo {
            role: "ROLE_ADMIN".to_string(),
            username: "admin".to_string(),
        };
        let json = serde_json::to_string(&role).unwrap();
        assert!(json.contains("\"role\":\"ROLE_ADMIN\""));

        let deserialized: RoleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, "ROLE_ADMIN");
    }

    #[test]
    fn test_permission_info_serialization() {
        let perm = PermissionInfo {
            role: "developer".to_string(),
            resource: "public:*:config/*".to_string(),
            action: "r".to_string(),
        };
        let json = serde_json::to_string(&perm).unwrap();
        let deserialized: PermissionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, "developer");
        assert_eq!(deserialized.resource, "public:*:config/*");
    }

    #[test]
    fn test_namespace_info_default() {
        let ns = NamespaceInfo::default();
        assert!(ns.namespace_id.is_empty());
        assert!(ns.namespace_name.is_empty());
        assert!(ns.namespace_desc.is_empty());
        assert_eq!(ns.config_count, 0);
        assert_eq!(ns.quota, 0);
    }

    #[test]
    fn test_namespace_info_serialization() {
        let ns = NamespaceInfo {
            namespace_id: "prod".to_string(),
            namespace_name: "Production".to_string(),
            namespace_desc: "Production environment".to_string(),
            config_count: 42i64,
            quota: 200,
        };
        let json = serde_json::to_string(&ns).unwrap();
        assert!(json.contains("\"namespaceId\":\"prod\""));
        assert!(json.contains("\"namespaceName\":\"Production\""));
        assert!(json.contains("\"configCount\":42"));
    }

    #[test]
    fn test_config_storage_data_default() {
        let config = ConfigStorageData::default();
        assert_eq!(config.id, 0);
        assert!(config.data_id.is_empty());
        assert!(config.content.is_empty());
        assert!(config.md5.is_empty());
    }

    #[test]
    fn test_config_storage_data_serialization() {
        let config = ConfigStorageData {
            id: 1,
            data_id: "app.yaml".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            tenant: "public".to_string(),
            content: "key: value".to_string(),
            md5: "abc123".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ConfigStorageData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.data_id, "app.yaml");
        assert_eq!(deserialized.content, "key: value");
    }

    #[test]
    fn test_config_history_storage_data_default() {
        let history = ConfigHistoryStorageData::default();
        assert_eq!(history.id, 0);
        assert!(history.op_type.is_empty());
    }

    #[test]
    fn test_capacity_info_default() {
        let cap = CapacityInfo::default();
        assert!(cap.id.is_none());
        assert!(cap.identifier.is_empty());
        assert_eq!(cap.quota, 0);
        assert_eq!(cap.usage, 0);
    }

    #[test]
    fn test_capacity_info_serialization() {
        let cap = CapacityInfo {
            id: Some(1),
            identifier: "public".to_string(),
            quota: 200,
            usage: 50,
            max_size: 102400,
            max_aggr_count: 10000,
            max_aggr_size: 2097152,
            max_history_count: 24,
        };
        let json = serde_json::to_string(&cap).unwrap();
        let deserialized: CapacityInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.quota, 200);
        assert_eq!(deserialized.usage, 50);
    }

    #[test]
    fn test_config_gray_storage_data_default() {
        let gray = ConfigGrayStorageData::default();
        assert!(gray.data_id.is_empty());
        assert!(gray.gray_name.is_empty());
        assert!(gray.gray_rule.is_empty());
    }

    #[test]
    fn test_storage_mode_serialization() {
        let mode = StorageMode::ExternalDb;
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: StorageMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, StorageMode::ExternalDb);
    }

    #[test]
    fn test_storage_mode_all_variants_roundtrip() {
        for mode in [
            StorageMode::ExternalDb,
            StorageMode::StandaloneEmbedded,
            StorageMode::DistributedEmbedded,
        ] {
            let s = mode.to_string();
            let parsed: StorageMode = s.parse().unwrap();
            assert_eq!(parsed, mode);
        }
    }
}
