//! CMDB Plugin Data Models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CMDB entity types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CmdbEntityType {
    /// Service entity
    Service,
    /// Instance entity
    Instance,
    /// Configuration entity
    Config,
    /// Namespace entity
    Namespace,
    /// Host/Server entity
    Host,
    /// Application entity
    Application,
    /// Environment entity
    Environment,
    /// Custom entity type
    Custom(String),
}

impl Default for CmdbEntityType {
    fn default() -> Self {
        Self::Service
    }
}

impl std::fmt::Display for CmdbEntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmdbEntityType::Service => write!(f, "service"),
            CmdbEntityType::Instance => write!(f, "instance"),
            CmdbEntityType::Config => write!(f, "config"),
            CmdbEntityType::Namespace => write!(f, "namespace"),
            CmdbEntityType::Host => write!(f, "host"),
            CmdbEntityType::Application => write!(f, "application"),
            CmdbEntityType::Environment => write!(f, "environment"),
            CmdbEntityType::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// CMDB entity representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmdbEntity {
    /// Entity ID (from Batata: namespace::group::name)
    pub id: String,
    /// Entity type
    pub entity_type: CmdbEntityType,
    /// Entity name
    pub name: String,
    /// Namespace
    #[serde(default)]
    pub namespace: String,
    /// Group (for services/configs)
    #[serde(default)]
    pub group: String,
    /// Entity labels
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Entity attributes (additional metadata)
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
    /// Parent entity ID
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Child entity IDs
    #[serde(default)]
    pub children: Vec<String>,
    /// External ID (in CMDB system)
    #[serde(default)]
    pub external_id: Option<String>,
    /// Last sync timestamp
    #[serde(default)]
    pub last_sync: i64,
    /// Entity status
    #[serde(default)]
    pub status: CmdbEntityStatus,
}

impl Default for CmdbEntity {
    fn default() -> Self {
        Self {
            id: String::new(),
            entity_type: CmdbEntityType::Service,
            name: String::new(),
            namespace: "public".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            labels: HashMap::new(),
            attributes: HashMap::new(),
            parent_id: None,
            children: Vec::new(),
            external_id: None,
            last_sync: 0,
            status: CmdbEntityStatus::Active,
        }
    }
}

impl CmdbEntity {
    pub fn new(entity_type: CmdbEntityType, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entity_type,
            ..Default::default()
        }
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.attributes.insert(key.into(), value);
        self
    }

    /// Generate entity ID from namespace, group, and name
    pub fn generate_id(&mut self) {
        self.id = format!("{}::{}::{}", self.namespace, self.group, self.name);
    }
}

/// CMDB entity status
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CmdbEntityStatus {
    /// Entity is active
    #[default]
    Active,
    /// Entity is inactive
    Inactive,
    /// Entity is pending sync
    Pending,
    /// Entity sync failed
    Failed,
    /// Entity is deleted
    Deleted,
}

/// CMDB label for tagging entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmdbLabel {
    /// Label key
    pub key: String,
    /// Label value
    pub value: String,
    /// Whether this is a system label
    #[serde(default)]
    pub system: bool,
    /// Label description
    #[serde(default)]
    pub description: String,
    /// Valid values (if restricted)
    #[serde(default)]
    pub allowed_values: Vec<String>,
    /// Whether the label is required
    #[serde(default)]
    pub required: bool,
}

impl CmdbLabel {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            system: false,
            description: String::new(),
            allowed_values: Vec::new(),
            required: false,
        }
    }

    pub fn system_label(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            system: true,
            description: String::new(),
            allowed_values: Vec::new(),
            required: false,
        }
    }
}

/// Label mapping configuration for syncing labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelMapping {
    /// Mapping ID
    pub id: String,
    /// Source label key (in Batata)
    pub source_key: String,
    /// Target label key (in CMDB)
    pub target_key: String,
    /// Value transformation
    #[serde(default)]
    pub transform: LabelTransform,
    /// Default value if source is missing
    #[serde(default)]
    pub default_value: Option<String>,
    /// Entity types this mapping applies to
    #[serde(default)]
    pub entity_types: Vec<CmdbEntityType>,
    /// Whether mapping is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for LabelMapping {
    fn default() -> Self {
        Self {
            id: String::new(),
            source_key: String::new(),
            target_key: String::new(),
            transform: LabelTransform::None,
            default_value: None,
            entity_types: Vec::new(),
            enabled: true,
        }
    }
}

/// Label value transformation
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelTransform {
    /// No transformation
    #[default]
    None,
    /// Convert to lowercase
    Lowercase,
    /// Convert to uppercase
    Uppercase,
    /// Prefix with a value
    Prefix(String),
    /// Suffix with a value
    Suffix(String),
    /// Replace pattern (simple string replace)
    Replace { from: String, to: String },
    /// Map values using a lookup table
    Map(HashMap<String, String>),
}

impl LabelTransform {
    pub fn apply(&self, value: &str) -> String {
        match self {
            LabelTransform::None => value.to_string(),
            LabelTransform::Lowercase => value.to_lowercase(),
            LabelTransform::Uppercase => value.to_uppercase(),
            LabelTransform::Prefix(prefix) => format!("{}{}", prefix, value),
            LabelTransform::Suffix(suffix) => format!("{}{}", value, suffix),
            LabelTransform::Replace { from, to } => value.replace(from, to),
            LabelTransform::Map(mapping) => mapping
                .get(value)
                .cloned()
                .unwrap_or_else(|| value.to_string()),
        }
    }
}

/// CMDB plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmdbConfig {
    /// Whether the CMDB plugin is enabled
    #[serde(default)]
    pub enabled: bool,
    /// CMDB provider type
    #[serde(default)]
    pub provider: CmdbProvider,
    /// CMDB API endpoint
    #[serde(default)]
    pub endpoint: String,
    /// Authentication token/key
    #[serde(default)]
    pub auth_token: String,
    /// Sync interval in seconds
    #[serde(default = "default_sync_interval")]
    pub sync_interval_seconds: u64,
    /// Sync direction
    #[serde(default)]
    pub sync_direction: SyncDirection,
    /// Label mappings
    #[serde(default)]
    pub label_mappings: Vec<LabelMapping>,
    /// Entity types to sync
    #[serde(default)]
    pub entity_types: Vec<CmdbEntityType>,
    /// Namespaces to sync (empty = all)
    #[serde(default)]
    pub namespaces: Vec<String>,
    /// Request timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_sync_interval() -> u64 {
    300
}

fn default_timeout() -> u64 {
    10000
}

impl Default for CmdbConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: CmdbProvider::None,
            endpoint: String::new(),
            auth_token: String::new(),
            sync_interval_seconds: 300,
            sync_direction: SyncDirection::BatataToOmdb,
            label_mappings: Vec::new(),
            entity_types: vec![CmdbEntityType::Service, CmdbEntityType::Instance],
            namespaces: Vec::new(),
            timeout_ms: 10000,
        }
    }
}

/// CMDB provider types
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CmdbProvider {
    /// No provider (disabled)
    #[default]
    None,
    /// ServiceNow CMDB
    ServiceNow,
    /// BMC Helix CMDB
    BmcHelix,
    /// Consul Catalog
    Consul,
    /// Kubernetes
    Kubernetes,
    /// Custom HTTP API
    Custom,
}

/// Sync direction for CMDB integration
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncDirection {
    /// Sync from Batata to CMDB
    #[default]
    BatataToOmdb,
    /// Sync from CMDB to Batata
    CmdbToBatata,
    /// Bidirectional sync
    Bidirectional,
}

/// Result of a CMDB sync operation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CmdbSyncResult {
    /// Whether the sync was successful
    pub success: bool,
    /// Number of entities synced
    pub synced: u32,
    /// Number of entities created
    pub created: u32,
    /// Number of entities updated
    pub updated: u32,
    /// Number of entities deleted
    pub deleted: u32,
    /// Number of errors
    pub errors: u32,
    /// Error messages
    pub error_messages: Vec<String>,
    /// Sync duration in milliseconds
    pub duration_ms: u64,
    /// Timestamp of sync
    pub timestamp: i64,
}

/// CMDB plugin statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CmdbStats {
    /// Total entities tracked
    pub total_entities: u32,
    /// Entities by type
    pub entities_by_type: HashMap<String, u32>,
    /// Total syncs performed
    pub total_syncs: u64,
    /// Successful syncs
    pub successful_syncs: u64,
    /// Failed syncs
    pub failed_syncs: u64,
    /// Last sync timestamp
    pub last_sync: i64,
    /// Last sync result
    pub last_sync_result: Option<CmdbSyncResult>,
}
