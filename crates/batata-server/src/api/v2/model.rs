//! Request and response models for V2 API
//!
//! These models follow the Nacos V2 API specification with camelCase JSON serialization.

use serde::{Deserialize, Serialize};

/// Default namespace ID used when none is specified
pub const DEFAULT_NAMESPACE_ID: &str = "public";

/// Default group name used when none is specified
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

// Re-export config/history V2 model types from batata-config
pub use batata_config::api::v2::model::{
    ConfigDeleteParam, ConfigGetParam, ConfigHistoryInfoDetail, ConfigInfoResponse,
    ConfigPublishParam, ConfigResponse, ConfigSearchDetailParam, HistoryDetailParam,
    HistoryItemResponse, HistoryListParam, HistoryPreviousParam, NamespaceConfigsParam,
};

// Re-export naming V2 model types from batata-naming
pub use batata_naming::api::v2::model::{
    BatchMetadataParam, ClientDetailParam, ClientDetailResponse, ClientListParam,
    ClientListResponse, ClientServiceInfo, ClientServiceListParam, ClientServiceListResponse,
    InstanceDeregisterParam, InstanceDetailParam, InstanceHealthParam, InstanceListParam,
    InstanceListResponse, InstanceRegisterParam, InstanceResponse, InstanceUpdateParam,
    NamingMetricsResponse, SelectorResponse, ServiceClientInfo, ServiceClientListParam,
    ServiceClientListResponse, ServiceCreateParam, ServiceDeleteParam, ServiceDetailParam,
    ServiceDetailResponse, ServiceListParam, ServiceListResponse, ServiceUpdateParam,
    SwitchUpdateParam, SwitchesResponse,
};

// =============================================================================
// Cluster API Models (to be moved to batata-core in Phase 5)
// =============================================================================

/// Request parameters for node list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeListParam {
    /// Filter by address prefix (optional)
    #[serde(default)]
    pub address: Option<String>,
    /// Filter by node state (optional)
    #[serde(default)]
    pub state: Option<String>,
}

/// Request parameters for switching lookup mode
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupSwitchParam {
    /// Lookup type: "file" or "address-server"
    pub r#type: String,
}

/// Response data for current node (self)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSelfResponse {
    /// Node IP address
    pub ip: String,
    /// Node port
    pub port: u16,
    /// Full address (ip:port)
    pub address: String,
    /// Node state (UP, DOWN, STARTING, etc.)
    pub state: String,
    /// Extended information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// Fail access count
    pub fail_access_cnt: i32,
    /// Abilities of this node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abilities: Option<NodeAbilities>,
}

/// Node abilities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeAbilities {
    /// Naming service abilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naming_ability: Option<NamingAbility>,
    /// Config service abilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_ability: Option<ConfigAbility>,
}

/// Naming service abilities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingAbility {
    /// Support gRPC push
    pub support_push: bool,
    /// Support remote delta updates
    pub support_delta_push: bool,
}

/// Config service abilities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAbility {
    /// Support remote config
    pub support_remote: bool,
}

/// Response data for node in list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeResponse {
    /// Node IP address
    pub ip: String,
    /// Node port
    pub port: u16,
    /// Full address (ip:port)
    pub address: String,
    /// Node state (UP, DOWN, STARTING, etc.)
    pub state: String,
    /// Extended information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// Fail access count
    pub fail_access_cnt: i32,
}

/// Response data for node health
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeHealthResponse {
    /// Whether this node is healthy
    pub healthy: bool,
}

/// Response data for lookup switch
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupSwitchResponse {
    /// Whether the switch was successful
    pub success: bool,
    /// Current lookup type
    pub current_type: String,
}
