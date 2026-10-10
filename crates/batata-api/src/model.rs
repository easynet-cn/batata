//! Common API models and constants
//!
//! This module defines shared constants, data structures, and enums
//! used across different API modules.

use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    str::FromStr,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// Client protocol version
/// The client protocol version.
pub const CLIENT_VERSION: &str = "3.0.0";
/// The protocol version that transmits data in the request body.
pub const DATA_IN_BODY_VERSION: i32 = 204;

// Header and parameter keys
/// Header key: application name.
pub const APPNAME: &str = "AppName";
/// Header key: client version.
pub const CLIENT_VERSION_KEY: &str = "ClientVersion";
/// Header key: client IP.
pub const CLIENT_IP: &str = "ClientIp";
/// Placeholder for an unknown application name.
pub const UNKNOWN_APP: &str = "UnknownApp";
/// Parameter key: data ID.
pub const DATA_ID: &str = "dataId";
/// Parameter key: tenant.
pub const TENANT: &str = "tenant";
/// Parameter key: group.
pub const GROUP: &str = "group";
/// Parameter key: group name (alternate key).
pub const GROUP_NAME: &str = "groupName";
/// Parameter key: namespace ID.
pub const NAMESPACE_ID: &str = "namespaceId";
/// Parameter key: target namespace ID.
pub const TARGET_NAMESPACE_ID: &str = "targetNamespaceId";

// HTTP headers
/// HTTP header: last modified time.
pub const LAST_MODIFIED: &str = "Last-Modified";
/// HTTP header: accepted encodings.
pub const ACCEPT_ENCODING: &str = "Accept-Encoding";
/// HTTP header: content encoding.
pub const CONTENT_ENCODING: &str = "Content-Encoding";
/// HTTP header: config listening probe request.
pub const PROBE_MODIFY_REQUEST: &str = "Listening-Configs";
/// HTTP header: config probe modify response.
pub const PROBE_MODIFY_RESPONSE: &str = "Probe-Modify-Response";
/// HTTP header: new config probe modify response.
pub const PROBE_MODIFY_RESPONSE_NEW: &str = "Probe-Modify-Response-New";
/// Value indicating zipped (compressed) content.
pub const USE_ZIP: &str = "true";
/// HTTP header: content MD5 checksum.
pub const CONTENT_MD5: &str = "Content-MD5";
/// HTTP header: config version.
pub const CONFIG_VERSION: &str = "Config-Version";
/// HTTP header: config type.
pub const CONFIG_TYPE: &str = "Config-Type";
/// HTTP header: encrypted data key.
pub const ENCRYPTED_DATA_KEY: &str = "Encrypted-Data-Key";
/// HTTP header: if-modified-since.
pub const IF_MODIFIED_SINCE: &str = "If-Modified-Since";
/// HTTP header: client spacing interval.
pub const SPACING_INTERVAL: &str = "client-spacing-interval";
/// HTTP header: client application name.
pub const CLIENT_APPNAME_HEADER: &str = "Client-AppName";
/// HTTP header: client request timestamp.
pub const CLIENT_REQUEST_TS_HEADER: &str = "Client-RequestTS";
/// HTTP header: client request token.
pub const CLIENT_REQUEST_TOKEN_HEADER: &str = "Client-RequestToken";
/// HTTP header: VIP server tag.
pub const VIPSERVER_TAG: &str = "Vipserver-Tag";
/// HTTP header: Amory tag.
pub const AMORY_TAG: &str = "Amory-Tag";
/// HTTP header: location tag.
pub const LOCATION_TAG: &str = "Location-Tag";
/// HTTP header: charset.
pub const CHARSET_KEY: &str = "charset";
/// HTTP header: notify flag.
pub const NOTIFY_HEADER: &str = "notify";

// API paths
/// Base path for config service APIs.
pub const BASE_PATH: &str = "/v1/cs";
/// Path for config controller APIs.
pub const CONFIG_CONTROLLER_PATH: &str = "/v1/cs/configs";

// Auth tokens
/// Parameter key: token.
pub const TOKEN: &str = "token";
/// Parameter key: access token.
pub const ACCESS_TOKEN: &str = "accessToken";
/// Parameter key: token TTL.
pub const TOKEN_TTL: &str = "tokenTtl";
/// Parameter key: global admin flag.
pub const GLOBAL_ADMIN: &str = "globalAdmin";
/// Parameter key: username.
pub const USERNAME: &str = "username";
/// Parameter key: token refresh window.
pub const TOKEN_REFRESH_WINDOW: &str = "tokenRefreshWindow";

// Port offsets
/// Default port offset for the SDK gRPC service.
pub const SDK_GRPC_PORT_DEFAULT_OFFSET: u16 = 1000;
/// Default port offset for the cluster gRPC service.
pub const CLUSTER_GRPC_PORT_DEFAULT_OFFSET: u16 = 1001;

// Timeouts and intervals
/// Async address update interval in seconds.
pub const ASYNC_UPDATE_ADDRESS_INTERVAL: i32 = 300;
/// Polling interval time in seconds.
pub const POLLING_INTERVAL_TIME: i32 = 15;
/// One-time operation timeout in milliseconds.
pub const ONCE_TIMEOUT: i64 = 2000;
/// Socket timeout in milliseconds.
pub const SO_TIMEOUT: i64 = 60000;
/// Config long-poll timeout in milliseconds.
pub const CONFIG_LONG_POLL_TIMEOUT: i64 = 30000;
/// Minimum config long-poll timeout in milliseconds.
pub const MIN_CONFIG_LONG_POLL_TIMEOUT: i64 = 10000;
/// Config retry interval in milliseconds.
pub const CONFIG_RETRY_TIME: i64 = 2000;
/// Maximum retry count.
pub const MAX_RETRY: i32 = 3;
/// Receive wait timeout in milliseconds.
pub const RECV_WAIT_TIMEOUT: i64 = ONCE_TIMEOUT * 5;
/// Default heartbeat timeout in milliseconds.
pub const DEFAULT_HEART_BEAT_TIMEOUT: i64 = 15 * 1000;
/// Default IP delete timeout in milliseconds.
pub const DEFAULT_IP_DELETE_TIMEOUT: i64 = 30 * 1000;
/// Default heartbeat interval in milliseconds.
pub const DEFAULT_HEART_BEAT_INTERVAL: i64 = 5 * 1000;
/// Default redo delay in milliseconds.
pub const DEFAULT_REDO_DELAY_TIME: i64 = 3000;
/// Default redo thread count.
pub const DEFAULT_REDO_THREAD_COUNT: i32 = 1;

// Flow control
/// Flow control threshold.
pub const FLOW_CONTROL_THRESHOLD: i32 = 20;
/// Flow control slot size.
pub const FLOW_CONTROL_SLOT: i32 = 10;
/// Flow control interval in milliseconds.
pub const FLOW_CONTROL_INTERVAL: i32 = 1000;
/// Default protection threshold for service health.
pub const DEFAULT_PROTECT_THRESHOLD: f32 = 0.0;
/// Maximum atomic batch size.
pub const ATOMIC_MAX_SIZE: i32 = 1000;

// Separators
/// Line separator used in batched payloads.
pub const LINE_SEPARATOR: &str = "\u{1}";
/// Word separator used in batched payloads.
pub const WORD_SEPARATOR: &str = "\u{2}";
/// Line separator for long-polling responses.
pub const LONGPOLLING_LINE_SEPARATOR: &str = "\r\n";
/// Separator between service info fields.
pub const SERVICE_INFO_SPLITER: &str = "@@";
/// Expected number of segments when splitting service info.
pub const SERVICE_INFO_SPLIT_COUNT: i32 = 2;
/// Separator in naming instance IDs.
pub const NAMING_INSTANCE_ID_SPLITTER: &str = "#";
/// Expected number of segments in a naming instance ID.
pub const NAMING_INSTANCE_ID_SEG_COUNT: i32 = 4;
/// Separator in naming HTTP header values.
pub const NAMING_HTTP_HEADER_SPLITTER: &str = "\\|";
/// Separator between fuzzy watch patterns.
pub const FUZZY_WATCH_PATTERN_SPLITTER: &str = ">>";
/// Colon separator.
pub const COLON: &str = ":";
/// Line break.
pub const LINE_BREAK: &str = "\n";
/// Pound (hash) separator.
pub const POUND: &str = "#";
/// Dot separator.
pub const DOT: &str = ".";

// Weight validation constants
/// Maximum allowed instance weight.
pub const MAX_WEIGHT_VALUE: f64 = 10000.0;
/// Minimum positive instance weight.
pub const MIN_POSITIVE_WEIGHT_VALUE: f64 = 0.01;
/// Minimum instance weight (zero).
pub const MIN_WEIGHT_VALUE: f64 = 0.0;
/// Default instance weight.
pub const DEFAULT_INSTANCE_WEIGHT: f64 = 1.0;

// Default values
/// Default cluster name.
pub const DEFAULT_CLUSTER_NAME: &str = "DEFAULT";
/// Whether to parse cloud namespaces by default.
pub const DEFAULT_USE_CLOUD_NAMESPACE_PARSING: bool = true;
/// Default value for RAM info parsing.
pub const DEFAULT_USE_RAM_INFO_PARSING: &str = "true";
/// Default instance ID generator.
pub const DEFAULT_INSTANCE_ID_GENERATOR: &str = "simple";
/// Snowflake instance ID generator.
pub const SNOWFLAKE_INSTANCE_ID_GENERATOR: &str = "snowflake";

// Patterns
/// Regex matching a positive integer.
pub const NUMBER_PATTERN_STRING: &str = "^\\d+$";
/// Regex matching any string.
pub const ANY_PATTERN: &str = ".*";
/// Pattern matching all values.
pub const ALL_PATTERN: &str = "*";
/// Regex validating a cluster name.
pub const CLUSTER_NAME_PATTERN_STRING: &str = "^[0-9a-zA-Z-]+$";

// Domain names
/// Default config domain name.
pub const DEFAULT_DOMAINNAME: &str = "commonconfig.config-host.taobao.com";
/// Daily config domain name.
pub const DAILY_DOMAINNAME: &str = "commonconfig.taobao.net";
/// Empty string constant.
pub const NULL: &str = "";
/// String representation of null.
pub const NULL_STRING: &str = "null";
/// Default character encoding.
pub const ENCODE: &str = "UTF-8";
/// Map file name.
pub const MAP_FILE: &str = "map-file.js";
/// HTTP prefix.
pub const HTTP_PREFIX: &str = "http";

// Redirect codes
/// HTTP redirect status code.
pub const WRITE_REDIRECT_CODE: i32 = 307;

// Module types
/// Module type key: client module type.
pub const CLIENT_MODULE_TYPE: &str = "clientModuleType";
/// Module name: config.
pub const CONFIG_MODULE: &str = "config";
/// Module name: naming.
pub const NAMING_MODULE: &str = "naming";
/// Module name: lock.
pub const LOCK_MODULE: &str = "lock";
/// Module name: internal.
pub const INTERNAL_MODULE: &str = "internal";
/// Module name: AI.
pub const AI_MODULE: &str = "ai";
/// Context type: CMDB.
pub const CMDB_CONTEXT_TYPE: &str = "CMDB";

// Connection labels
/// Label key for application connection labels.
pub const APP_CONN_LABELS_KEY: &str = "batata.app.conn.labels";
/// Label key for preferred connection labels.
pub const APP_CONN_LABELS_PREFERRED: &str = "nacos_app_conn_labels_preferred";
/// Prefix for application connection labels.
pub const APP_CONN_PREFIX: &str = "app_";
/// Label key for config gray.
pub const CONFIG_GRAY_LABEL: &str = "batata.config.gray.label";
/// Label key: instance weight.
pub const WEIGHT: &str = "weight";
/// Label key: properties.
pub const PROPERTIES_KEY: &str = "properties";
/// Label key: JVM info.
pub const JVM_KEY: &str = "jvm";
/// Label key: environment info.
pub const ENV_KEY: &str = "env";

// Fuzzy watch types
/// Fuzzy watch event: initial notify.
pub const FUZZY_WATCH_INIT_NOTIFY: &str = "FUZZY_WATCH_INIT_NOTIFY";
/// Fuzzy watch event: finish initial notify.
pub const FINISH_FUZZY_WATCH_INIT_NOTIFY: &str = "FINISH_FUZZY_WATCH_INIT_NOTIFY";
/// Fuzzy watch event: diff sync notify.
pub const FUZZY_WATCH_DIFF_SYNC_NOTIFY: &str = "FUZZY_WATCH_DIFF_SYNC_NOTIFY";
/// Fuzzy watch event: resource changed.
pub const FUZZY_WATCH_RESOURCE_CHANGED: &str = "FUZZY_WATCH_RESOURCE_CHANGED";
/// Watch action: start watching.
pub const WATCH_TYPE_WATCH: &str = "WATCH";
/// Watch action: cancel watching.
pub const WATCH_TYPE_CANCEL_WATCH: &str = "CANCEL_WATCH";

// Event types
/// Event: add config.
pub const ADD_CONFIG: &str = "ADD_CONFIG";
/// Event: delete config.
pub const DELETE_CONFIG: &str = "DELETE_CONFIG";
/// Event: config changed.
pub const CONFIG_CHANGED: &str = "CONFIG_CHANGED";
/// Event: add service.
pub const ADD_SERVICE: &str = "ADD_SERVICE";
/// Event: delete service.
pub const DELETE_SERVICE: &str = "DELETE_SERVICE";
/// Event: instance changed.
pub const INSTANCE_CHANGED: &str = "INSTANCE_CHANGED";
/// Event: heartbeat.
pub const HEART_BEAT: &str = "HEART_BEAT";

// Error codes
/// Serialization error code.
pub const SERIALIZE_ERROR_CODE: i32 = 100;
/// Deserialization error code.
pub const DESERIALIZE_ERROR_CODE: i32 = 101;
/// Data source lookup error code.
pub const FIND_DATASOURCE_ERROR_CODE: i32 = 102;
/// Table lookup error code.
pub const FIND_TABLE_ERROR_CODE: i32 = 103;

/// Generic pagination wrapper for API responses
///
/// Serde aliases support Nacos-compatible deserialization where different
/// endpoints use different field names for the same concept.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    #[serde(alias = "count", default)]
    /// The `total_count` field.
    pub total_count: u64,
    #[serde(default)]
    /// The `page_number` field.
    pub page_number: u64,
    #[serde(default)]
    /// The `pages_available` field.
    pub pages_available: u64,
    #[serde(
        alias = "serviceList",
        alias = "configList",
        alias = "hosts",
        alias = "subscribers",
        alias = "list",
        default
    )]
    /// The `page_items` field.
    pub page_items: Vec<T>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            total_count: 0,
            page_number: 1,
            pages_available: 0,
            page_items: vec![],
        }
    }
}

impl<T> Page<T> {
    /// Creates a new `Page` from the given counts and items.
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

    /// Creates an empty `Page`.
    pub fn empty() -> Self {
        Self::default()
    }
}

/// Node state enumeration for cluster members
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum NodeState {
    /// The `variant` variant.
    Starting,
    #[default]
    /// The `variant` variant.
    Up,
    /// The `variant` variant.
    Suspicious,
    /// The `variant` variant.
    Down,
    /// The `variant` variant.
    Isolation,
}

impl NodeState {
    /// The `as_str` method.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeState::Starting => "STARTING",
            NodeState::Up => "UP",
            NodeState::Suspicious => "SUSPICIOUS",
            NodeState::Down => "DOWN",
            NodeState::Isolation => "ISOLATION",
        }
    }

    /// Returns whether is healthy.
    pub fn is_healthy(&self) -> bool {
        matches!(self, NodeState::Up)
    }
}

impl Display for NodeState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for NodeState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "STARTING" => Ok(NodeState::Starting),
            "UP" => Ok(NodeState::Up),
            "SUSPICIOUS" => Ok(NodeState::Suspicious),
            "DOWN" => Ok(NodeState::Down),
            "ISOLATION" => Ok(NodeState::Isolation),
            _ => Err(format!("Invalid node state: {}", s)),
        }
    }
}

/// Cluster member information structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// The `ip` field.
    pub ip: String,
    /// The `port` field.
    pub port: u16,
    /// The `state` field.
    pub state: NodeState,
    #[serde(skip)]
    /// The `extend_info` field.
    pub extend_info: Arc<RwLock<BTreeMap<String, serde_json::Value>>>,
    /// The `address` field.
    pub address: String,
    /// The `fail_access_cnt` field.
    pub fail_access_cnt: i32,
}

impl Member {
    /// The `RAFT_PORT` constant.
    pub const RAFT_PORT: &str = "raftPort";
    /// The `SITE_KEY` constant.
    pub const SITE_KEY: &str = "site";
    /// The `AD_WEIGHT` constant.
    pub const AD_WEIGHT: &str = "adWeight";
    /// The `WEIGHT` constant.
    pub const WEIGHT: &str = "weight";
    /// The `LAST_REFRESH_TIME` constant.
    pub const LAST_REFRESH_TIME: &str = "lastRefreshTime";
    /// The `VERSION` constant.
    pub const VERSION: &str = "version";
    /// The `SUPPORT_REMOTE_C_TYPE` constant.
    pub const SUPPORT_REMOTE_C_TYPE: &str = "remoteConnectType";
    /// The `READY_TO_UPGRADE` constant.
    pub const READY_TO_UPGRADE: &str = "readyToUpgrade";
    /// The `SUPPORT_GRAY_MODEL` constant.
    pub const SUPPORT_GRAY_MODEL: &str = "supportGrayModel";

    // Multi-datacenter support constants
    /// The `DATACENTER` constant.
    pub const DATACENTER: &str = "datacenter";
    /// The `REGION` constant.
    pub const REGION: &str = "region";
    /// The `ZONE` constant.
    pub const ZONE: &str = "zone";
    /// The `LOCALITY_WEIGHT` constant.
    pub const LOCALITY_WEIGHT: &str = "localityWeight";
    /// The `CLUSTER_GROUP` constant.
    pub const CLUSTER_GROUP: &str = "clusterGroup";
    /// The `DEFAULT_DATACENTER` constant.
    pub const DEFAULT_DATACENTER: &str = "default";
    /// The `DEFAULT_REGION` constant.
    pub const DEFAULT_REGION: &str = "default";
    /// The `DEFAULT_ZONE` constant.
    pub const DEFAULT_ZONE: &str = "default";

    /// The `TARGET_MEMBER_CONNECT_REFUSE_ERRMSG` constant.
    pub const TARGET_MEMBER_CONNECT_REFUSE_ERRMSG: &str = "Connection refused";
    /// The `SERVER_PORT_PROPERTY` constant.
    pub const SERVER_PORT_PROPERTY: &str = "batata.server.main.port";
    /// The `DEFAULT_SERVER_PORT` constant.
    pub const DEFAULT_SERVER_PORT: u16 = 8848;
    /// The `DEFAULT_RAFT_OFFSET_PORT` constant.
    pub const DEFAULT_RAFT_OFFSET_PORT: u16 = 1000;
    /// The `MEMBER_FAIL_ACCESS_CNT_PROPERTY` constant.
    pub const MEMBER_FAIL_ACCESS_CNT_PROPERTY: &str = "batata.core.member.fail-access-cnt";
    /// The `DEFAULT_MEMBER_FAIL_ACCESS_CNT` constant.
    pub const DEFAULT_MEMBER_FAIL_ACCESS_CNT: i16 = 3;

    /// Creates a new instance.
    pub fn new(ip: String, port: u16) -> Self {
        Self {
            ip: ip.clone(),
            port,
            state: NodeState::Up,
            extend_info: Arc::new(RwLock::new(BTreeMap::new())),
            address: format!("{}:{}", ip, port),
            fail_access_cnt: 0,
        }
    }

    /// The `calculate_raft_port` method.
    pub fn calculate_raft_port(&self) -> u16 {
        self.port - Member::DEFAULT_RAFT_OFFSET_PORT
    }

    /// Returns whether is healthy.
    pub fn is_healthy(&self) -> bool {
        self.state.is_healthy()
    }

    /// Get the datacenter this member belongs to
    pub fn datacenter(&self) -> String {
        self.get_extend_info_string(Self::DATACENTER)
            .unwrap_or_else(|| Self::DEFAULT_DATACENTER.to_string())
    }

    /// Set the datacenter for this member
    pub fn set_datacenter(&self, datacenter: &str) {
        self.set_extend_info(
            Self::DATACENTER,
            serde_json::Value::String(datacenter.to_string()),
        );
    }

    /// Get the region this member belongs to
    pub fn region(&self) -> String {
        self.get_extend_info_string(Self::REGION)
            .unwrap_or_else(|| Self::DEFAULT_REGION.to_string())
    }

    /// Set the region for this member
    pub fn set_region(&self, region: &str) {
        self.set_extend_info(Self::REGION, serde_json::Value::String(region.to_string()));
    }

    /// Get the zone this member belongs to
    pub fn zone(&self) -> String {
        self.get_extend_info_string(Self::ZONE)
            .unwrap_or_else(|| Self::DEFAULT_ZONE.to_string())
    }

    /// Set the zone for this member
    pub fn set_zone(&self, zone: &str) {
        self.set_extend_info(Self::ZONE, serde_json::Value::String(zone.to_string()));
    }

    /// Get the locality weight (priority for local-first sync)
    pub fn locality_weight(&self) -> f64 {
        self.get_extend_info(Self::LOCALITY_WEIGHT)
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0)
    }

    /// Set the locality weight
    pub fn set_locality_weight(&self, weight: f64) {
        self.set_extend_info(Self::LOCALITY_WEIGHT, serde_json::json!(weight));
    }

    /// Get the cluster group (logical partition)
    pub fn cluster_group(&self) -> Option<String> {
        self.get_extend_info_string(Self::CLUSTER_GROUP)
    }

    /// Set the cluster group
    pub fn set_cluster_group(&self, group: &str) {
        self.set_extend_info(
            Self::CLUSTER_GROUP,
            serde_json::Value::String(group.to_string()),
        );
    }

    /// Check if this member is in the same datacenter
    pub fn is_same_datacenter(&self, other: &Member) -> bool {
        self.datacenter() == other.datacenter()
    }

    /// Check if this member is in the same region
    pub fn is_same_region(&self, other: &Member) -> bool {
        self.region() == other.region()
    }

    /// Check if this member is in the same zone
    pub fn is_same_zone(&self, other: &Member) -> bool {
        self.zone() == other.zone()
    }

    /// Get full locality path (region/datacenter/zone)
    pub fn locality_path(&self) -> String {
        format!("{}/{}/{}", self.region(), self.datacenter(), self.zone())
    }

    /// Helper to get extend info as string
    fn get_extend_info_string(&self, key: &str) -> Option<String> {
        self.get_extend_info(key)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
    }

    /// Helper to get extend info value
    fn get_extend_info(&self, key: &str) -> Option<serde_json::Value> {
        self.extend_info
            .read()
            .ok()
            .and_then(|info| info.get(key).cloned())
    }

    /// Helper to set extend info value
    fn set_extend_info(&self, key: &str, value: serde_json::Value) {
        if let Ok(mut info) = self.extend_info.write() {
            info.insert(key.to_string(), value);
        }
    }
}

/// Builder pattern for creating Member instances
pub struct MemberBuilder {
    ip: String,
    port: u16,
    node_state: NodeState,
    extend_info: Arc<RwLock<BTreeMap<String, serde_json::Value>>>,
}

impl MemberBuilder {
    /// Creates a new instance.
    pub fn new(ip: String, port: u16) -> Self {
        MemberBuilder {
            ip,
            port,
            node_state: NodeState::default(),
            extend_info: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// The `ip` method.
    pub fn ip(mut self, ip: String) -> Self {
        self.ip = ip;
        self
    }

    /// The `port` method.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// The `node_state` method.
    pub fn node_state(mut self, node_state: NodeState) -> Self {
        self.node_state = node_state;
        self
    }

    /// The `extend_info` method.
    pub fn extend_info(mut self, info: BTreeMap<String, Value>) -> Self {
        self.extend_info = Arc::new(RwLock::new(info));
        self
    }

    /// Set the datacenter for this member
    pub fn datacenter(self, datacenter: &str) -> Self {
        if let Ok(mut info) = self.extend_info.write() {
            info.insert(
                Member::DATACENTER.to_string(),
                Value::String(datacenter.to_string()),
            );
        }
        self
    }

    /// Set the region for this member
    pub fn region(self, region: &str) -> Self {
        if let Ok(mut info) = self.extend_info.write() {
            info.insert(
                Member::REGION.to_string(),
                Value::String(region.to_string()),
            );
        }
        self
    }

    /// Set the zone for this member
    pub fn zone(self, zone: &str) -> Self {
        if let Ok(mut info) = self.extend_info.write() {
            info.insert(Member::ZONE.to_string(), Value::String(zone.to_string()));
        }
        self
    }

    /// Set the locality weight
    pub fn locality_weight(self, weight: f64) -> Self {
        if let Ok(mut info) = self.extend_info.write() {
            info.insert(
                Member::LOCALITY_WEIGHT.to_string(),
                serde_json::json!(weight),
            );
        }
        self
    }

    /// Set the cluster group
    pub fn cluster_group(self, group: &str) -> Self {
        if let Ok(mut info) = self.extend_info.write() {
            info.insert(
                Member::CLUSTER_GROUP.to_string(),
                Value::String(group.to_string()),
            );
        }
        self
    }

    /// The `build` method.
    pub fn build(self) -> Member {
        Member {
            ip: self.ip.clone(),
            port: self.port,
            state: self.node_state,
            extend_info: self.extend_info,
            address: format!("{}:{}", self.ip, self.port),
            fail_access_cnt: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_default() {
        let page: Page<String> = Page::default();
        assert_eq!(page.total_count, 0);
        assert_eq!(page.page_number, 1);
        assert!(page.page_items.is_empty());
    }

    #[test]
    fn test_page_new() {
        let items = vec!["a".to_string(), "b".to_string()];
        let page = Page::new(10, 1, 5, items);
        assert_eq!(page.total_count, 10);
        assert_eq!(page.pages_available, 2);
    }

    #[test]
    fn test_node_state() {
        assert_eq!(NodeState::default(), NodeState::Up);
        assert!(NodeState::Up.is_healthy());
        assert!(!NodeState::Down.is_healthy());
    }

    #[test]
    fn test_member_builder() {
        let member = MemberBuilder::new("127.0.0.1".to_string(), 8848)
            .node_state(NodeState::Up)
            .build();
        assert_eq!(member.ip, "127.0.0.1");
        assert_eq!(member.port, 8848);
        assert_eq!(member.address, "127.0.0.1:8848");
    }

    #[test]
    fn test_page_pagination_calculation() {
        // 25 items, page 1, page size 10 -> 3 pages
        let page: Page<i32> = Page::new(25, 1, 10, vec![1, 2, 3]);
        assert_eq!(page.pages_available, 3);
        assert_eq!(page.total_count, 25);
        assert_eq!(page.page_number, 1);
    }

    #[test]
    fn test_page_exact_division() {
        let page: Page<i32> = Page::new(20, 1, 10, vec![]);
        assert_eq!(page.pages_available, 2);
    }

    #[test]
    fn test_page_single_item() {
        let page: Page<i32> = Page::new(1, 1, 10, vec![1]);
        assert_eq!(page.pages_available, 1);
    }

    #[test]
    fn test_page_zero_page_size() {
        let page: Page<i32> = Page::new(10, 1, 0, vec![]);
        assert_eq!(page.pages_available, 0);
    }

    #[test]
    fn test_node_state_all_variants() {
        let states = vec![
            NodeState::Starting,
            NodeState::Up,
            NodeState::Suspicious,
            NodeState::Down,
            NodeState::Isolation,
        ];
        for state in states {
            // Ensure Display and Debug work
            let _ = format!("{}", state);
            let _ = format!("{:?}", state);
        }
    }

    #[test]
    fn test_member_builder_with_ip_port() {
        let member = MemberBuilder::new("192.168.1.1".to_string(), 8848)
            .node_state(NodeState::Up)
            .build();
        assert_eq!(member.ip, "192.168.1.1");
        assert_eq!(member.port, 8848);
        assert_eq!(member.address, "192.168.1.1:8848");
    }

    #[test]
    fn test_member_address_format() {
        let member = MemberBuilder::new("10.0.0.1".to_string(), 9848).build();
        assert_eq!(member.address, "10.0.0.1:9848");
    }

    #[test]
    fn test_api_constants() {
        assert_eq!(super::CLIENT_VERSION, "3.0.0");
        assert_eq!(super::SDK_GRPC_PORT_DEFAULT_OFFSET, 1000);
        assert_eq!(super::CLUSTER_GRPC_PORT_DEFAULT_OFFSET, 1001);
    }

    #[test]
    fn test_page_serialization() {
        let page = Page::new(10, 1, 5, vec!["item1".to_string(), "item2".to_string()]);
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"totalCount\":10"));
        assert!(json.contains("\"pageNumber\":1"));
        assert!(json.contains("\"pagesAvailable\":2"));
    }
}
