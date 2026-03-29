//! Domain model types for the persistence abstraction layer
//!
//! These types are used as return values from the persistence traits,
//! decoupled from specific storage backends.

use serde::{Deserialize, Serialize};

/// Basic user information returned from persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub password: String,
    pub enabled: bool,
}

/// Role assignment information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    pub role: String,
    pub username: String,
}

/// Permission information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionInfo {
    pub role: String,
    pub resource: String,
    pub action: String,
}

// ============================================================================
// AI Resource persistence models
// ============================================================================

/// AI resource metadata (storage-agnostic representation of ai_resource row)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResourceInfo {
    pub id: i64,
    pub name: String,
    pub resource_type: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub namespace_id: String,
    pub biz_tags: Option<String>,
    pub ext: Option<String>,
    pub from: String,
    pub version_info: Option<String>,
    pub meta_version: i64,
    pub scope: String,
    pub owner: String,
    pub download_count: i64,
    pub gmt_create: Option<String>,
    pub gmt_modified: Option<String>,
}

/// AI resource version (storage-agnostic representation of ai_resource_version row)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResourceVersionInfo {
    pub id: i64,
    pub resource_type: String,
    pub author: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub version: String,
    pub namespace_id: String,
    pub storage: Option<String>,
    pub publish_pipeline_info: Option<String>,
    pub download_count: i64,
    pub gmt_create: Option<String>,
    pub gmt_modified: Option<String>,
}

/// Pipeline execution (storage-agnostic representation of pipeline_execution row)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineExecutionInfo {
    pub execution_id: String,
    pub resource_type: String,
    pub resource_name: String,
    pub namespace_id: Option<String>,
    pub version: Option<String>,
    pub status: String,
    pub pipeline: String,
    pub create_time: i64,
    pub update_time: i64,
}

/// Generic paginated result
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total_count: u64,
    pub page_number: u64,
    pub pages_available: u64,
    pub page_items: Vec<T>,
}

impl<T> Page<T> {
    pub fn new(total_count: u64, page_number: u64, page_size: u64, page_items: Vec<T>) -> Self {
        Self {
            total_count,
            page_number,
            pages_available: if page_size > 0 {
                (total_count as f64 / page_size as f64).ceil() as u64
            } else {
                0
            },
            page_items,
        }
    }

    pub fn empty() -> Self {
        Self {
            total_count: 0,
            page_number: 0,
            pages_available: 0,
            page_items: Vec::new(),
        }
    }
}

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

    /// Get the storage backend
    pub fn backend(&self) -> StorageBackend {
        match self {
            StorageMode::ExternalDb => StorageBackend::ExternalDb,
            StorageMode::StandaloneEmbedded | StorageMode::DistributedEmbedded => {
                StorageBackend::Embedded
            }
        }
    }

    /// Get the deploy topology
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
    pub namespace_id: String,
    pub namespace_name: String,
    pub namespace_desc: String,
    pub config_count: i32,
    pub quota: i32,
}

/// Config information stored in embedded backends
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfigStorageData {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub config_type: String,
    pub desc: String,
    pub r#use: String,
    pub effect: String,
    pub schema: String,
    pub config_tags: String,
    pub encrypted_data_key: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub modified_time: i64,
}

/// Config history entry stored in embedded backends
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfigHistoryStorageData {
    pub id: u64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub src_user: String,
    pub src_ip: String,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub ext_info: String,
    pub encrypted_data_key: String,
    pub created_time: i64,
    pub modified_time: i64,
}

/// Capacity information for tenant or group quotas
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CapacityInfo {
    /// Unique identifier
    pub id: Option<u64>,
    /// Tenant ID or Group ID
    pub identifier: String,
    /// Maximum number of configs allowed
    pub quota: u32,
    /// Current usage count
    pub usage: u32,
    /// Maximum config size in bytes
    pub max_size: u32,
    /// Maximum aggregate config count
    pub max_aggr_count: u32,
    /// Maximum aggregate config size
    pub max_aggr_size: u32,
    /// Maximum history count
    pub max_history_count: u32,
}

/// Gray config data stored in embedded backends
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfigGrayStorageData {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub gray_name: String,
    pub gray_rule: String,
    pub encrypted_data_key: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
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
            config_count: 42,
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
