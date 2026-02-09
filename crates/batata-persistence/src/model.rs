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

/// Storage mode for the persistence layer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageMode {
    /// External database (MySQL/PostgreSQL via SeaORM)
    ExternalDb,
    /// Standalone embedded RocksDB (single node, no external DB)
    StandaloneEmbedded,
    /// Distributed embedded RocksDB (Raft cluster, no external DB)
    DistributedEmbedded,
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
}
