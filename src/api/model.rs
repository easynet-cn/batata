// Common API models and constants for Batata application
// This file defines shared constants, data structures, and enums used across different API modules

use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    str::FromStr,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// Client protocol version
pub const CLIENT_VERSION: &str = "3.0.0";

pub const DATA_IN_BODY_VERSION: i32 = 204;

pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

pub const APPNAME: &str = "AppName";

pub const CLIENT_VERSION_KEY: &str = "ClientVersion";

pub const CLIENT_IP: &str = "ClientIp";

pub const UNKNOWN_APP: &str = "UnknownApp";

pub const DEFAULT_DOMAINNAME: &str = "commonconfig.config-host.taobao.com";

pub const DAILY_DOMAINNAME: &str = "commonconfig.taobao.net";

pub const NULL: &str = "";

pub const DATA_ID: &str = "dataId";

pub const TENANT: &str = "tenant";

pub const GROUP: &str = "group";

pub const GROUP_NAME: &str = "groupName";

pub const NAMESPACE_ID: &str = "namespaceId";

pub const TARGET_NAMESPACE_ID: &str = "targetNamespaceId";

pub const LAST_MODIFIED: &str = "Last-Modified";

pub const ACCEPT_ENCODING: &str = "Accept-Encoding";

pub const CONTENT_ENCODING: &str = "Content-Encoding";

pub const PROBE_MODIFY_REQUEST: &str = "Listening-Configs";

pub const PROBE_MODIFY_RESPONSE: &str = "Probe-Modify-Response";

pub const PROBE_MODIFY_RESPONSE_NEW: &str = "Probe-Modify-Response-New";

pub const USE_ZIP: &str = "true";

pub const CONTENT_MD5: &str = "Content-MD5";

pub const CONFIG_VERSION: &str = "Config-Version";

pub const CONFIG_TYPE: &str = "Config-Type";

pub const ENCRYPTED_DATA_KEY: &str = "Encrypted-Data-Key";

pub const IF_MODIFIED_SINCE: &str = "If-Modified-Since";

pub const SPACING_INTERVAL: &str = "client-spacing-interval";

pub const BASE_PATH: &str = "/v1/cs";

pub const CONFIG_CONTROLLER_PATH: &str = "/v1/cs/configs";

pub const TOKEN: &str = "token";

pub const ACCESS_TOKEN: &str = "accessToken";

pub const TOKEN_TTL: &str = "tokenTtl";

pub const GLOBAL_ADMIN: &str = "globalAdmin";

pub const USERNAME: &str = "username";

pub const TOKEN_REFRESH_WINDOW: &str = "tokenRefreshWindow";

pub const SDK_GRPC_PORT_DEFAULT_OFFSET: u16 = 1000;

pub const CLUSTER_GRPC_PORT_DEFAULT_OFFSET: u16 = 1001;

pub const ASYNC_UPDATE_ADDRESS_INTERVAL: i32 = 300;

pub const POLLING_INTERVAL_TIME: i32 = 15;

pub const ONCE_TIMEOUT: i64 = 2000;

pub const SO_TIMEOUT: i64 = 60000;

pub const CONFIG_LONG_POLL_TIMEOUT: i64 = 30000;

pub const MIN_CONFIG_LONG_POLL_TIMEOUT: i64 = 10000;

pub const CONFIG_RETRY_TIME: i64 = 2000;

pub const MAX_RETRY: i32 = 3;

pub const RECV_WAIT_TIMEOUT: i64 = ONCE_TIMEOUT * 5;

pub const ENCODE: &str = "UTF-8";

pub const MAP_FILE: &str = "map-file.js";

pub const FLOW_CONTROL_THRESHOLD: i32 = 20;

pub const FLOW_CONTROL_SLOT: i32 = 10;

pub const FLOW_CONTROL_INTERVAL: i32 = 1000;

pub const DEFAULT_PROTECT_THRESHOLD: f32 = 0.0;

pub const LINE_SEPARATOR: &str = "\u{1}";

pub const WORD_SEPARATOR: &str = "\u{2}";

pub const LONGPOLLING_LINE_SEPARATOR: &str = "\r\n";

pub const CLIENT_APPNAME_HEADER: &str = "Client-AppName";

pub const CLIENT_REQUEST_TS_HEADER: &str = "Client-RequestTS";

pub const CLIENT_REQUEST_TOKEN_HEADER: &str = "Client-RequestToken";

pub const ATOMIC_MAX_SIZE: i32 = 1000;

pub const NAMING_INSTANCE_ID_SPLITTER: &str = "#";

pub const NAMING_INSTANCE_ID_SEG_COUNT: i32 = 4;

pub const NAMING_HTTP_HEADER_SPLITTER: &str = "\\|";

pub const DEFAULT_CLUSTER_NAME: &str = "DEFAULT";

pub const DEFAULT_HEART_BEAT_TIMEOUT: i64 = 15 * 1000;

pub const DEFAULT_IP_DELETE_TIMEOUT: i64 = 30 * 1000;

pub const DEFAULT_HEART_BEAT_INTERVAL: i64 = 5 * 1000;

pub const DEFAULT_NAMESPACE_ID: &str = "public";

pub const DEFAULT_USE_CLOUD_NAMESPACE_PARSING: bool = true;

pub const WRITE_REDIRECT_CODE: i32 = 307;

pub const SERVICE_INFO_SPLITER: &str = "@@";

pub const SERVICE_INFO_SPLIT_COUNT: i32 = 2;

pub const NULL_STRING: &str = "null";

pub const NUMBER_PATTERN_STRING: &str = "^\\d+$";

pub const ANY_PATTERN: &str = ".*";

pub const DEFAULT_INSTANCE_ID_GENERATOR: &str = "simple";

pub const SNOWFLAKE_INSTANCE_ID_GENERATOR: &str = "snowflake";

pub const HTTP_PREFIX: &str = "http";

pub const ALL_PATTERN: &str = "*";

pub const COLON: &str = ":";

pub const LINE_BREAK: &str = "\n";

pub const POUND: &str = "#";

pub const VIPSERVER_TAG: &str = "Vipserver-Tag";

pub const AMORY_TAG: &str = "Amory-Tag";

pub const LOCATION_TAG: &str = "Location-Tag";

pub const CHARSET_KEY: &str = "charset";

pub const CLUSTER_NAME_PATTERN_STRING: &str = "^[0-9a-zA-Z-]+$";

pub const DEFAULT_REDO_DELAY_TIME: i64 = 3000;

pub const DEFAULT_REDO_THREAD_COUNT: i32 = 1;

pub const APP_CONN_LABELS_KEY: &str = "nacos.app.conn.labels";

pub const DOT: &str = ".";

pub const WEIGHT: &str = "weight";

pub const PROPERTIES_KEY: &str = "properties";

pub const JVM_KEY: &str = "jvm";

pub const ENV_KEY: &str = "env";

pub const APP_CONN_LABELS_PREFERRED: &str = "nacos_app_conn_labels_preferred";

pub const APP_CONN_PREFIX: &str = "app_";

pub const CONFIG_GRAY_LABEL: &str = "nacos.config.gray.label";

pub const DEFAULT_USE_RAM_INFO_PARSING: &str = "true";

pub const CLIENT_MODULE_TYPE: &str = "clientModuleType";

pub const CONFIG_MODULE: &str = "config";

pub const NOTIFY_HEADER: &str = "notify";

pub const NAMING_MODULE: &str = "naming";

pub const CMDB_CONTEXT_TYPE: &str = "CMDB";

pub const FUZZY_WATCH_PATTERN_SPLITTER: &str = ">>";

/**
 * fuzzy watch sync type of watch init notify.
 */
pub const FUZZY_WATCH_INIT_NOTIFY: &str = "FUZZY_WATCH_INIT_NOTIFY";

/**
 * fuzzy watch sync type of watch init notify finish.
 */
pub const FINISH_FUZZY_WATCH_INIT_NOTIFY: &str = "FINISH_FUZZY_WATCH_INIT_NOTIFY";

/**
 * fuzzy watch sync type of watch diff sync notify.
 */
pub const FUZZY_WATCH_DIFF_SYNC_NOTIFY: &str = "FUZZY_WATCH_DIFF_SYNC_NOTIFY";

/**
 * fuzzy watch sync type of watch resource changed.
 */
pub const FUZZY_WATCH_RESOURCE_CHANGED: &str = "FUZZY_WATCH_RESOURCE_CHANGED";

/**
 * watch type of watch.
 */
pub const WATCH_TYPE_WATCH: &str = "WATCH";

/**
 * watch type of cancel watch.
 */
pub const WATCH_TYPE_CANCEL_WATCH: &str = "CANCEL_WATCH";

pub const ADD_CONFIG: &str = "ADD_CONFIG";

pub const DELETE_CONFIG: &str = "DELETE_CONFIG";

pub const CONFIG_CHANGED: &str = "CONFIG_CHANGED";

pub const ADD_SERVICE: &str = "ADD_SERVICE";

pub const DELETE_SERVICE: &str = "DELETE_SERVICE";

pub const INSTANCE_CHANGED: &str = "INSTANCE_CHANGED";

pub const HEART_BEAT: &str = "HEART_BEAT";

pub const LOCK_MODULE: &str = "lock";

pub const INTERNAL_MODULE: &str = "internal";

pub const SERIALIZE_ERROR_CODE: i32 = 100;

pub const DESERIALIZE_ERROR_CODE: i32 = 101;

pub const FIND_DATASOURCE_ERROR_CODE: i32 = 102;

pub const FIND_TABLE_ERROR_CODE: i32 = 103;

pub const AI_MODULE: &str = "ai";

// Generic pagination wrapper for API responses
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total_count: u64,
    pub page_number: u64,
    pub pages_available: u64,
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
    pub fn new(total_count: u64, page_number: u64, page_size: u64, page_items: Vec<T>) -> Self {
        Self {
            total_count,
            page_number,
            pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
            page_items,
        }
    }
}

// Node state enumeration for cluster members
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[derive(Default)]
pub enum NodeState {
    Starting,
    #[default]
    Up,
    Suspicious,
    Down,
    Isolation,
}

impl NodeState {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeState::Starting => "STARTING",
            NodeState::Up => "UP",
            NodeState::Suspicious => "SUSPICIOUS",
            NodeState::Down => "DOWN",
            NodeState::Isolation => "ISOLATION",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "STARTING" => Ok(NodeState::Starting),
            "UP" => Ok(NodeState::Up),
            "SUSPICIOUS" => Ok(NodeState::Suspicious),
            "DOWN" => Ok(NodeState::Down),
            "htISOLATIONml" => Ok(NodeState::Isolation),
            _ => Err(format!("Invalid node state: {}", s)),
        }
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
        NodeState::from_str(s)
    }
}

// Cluster member information structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub ip: String,
    pub port: u16,
    pub state: NodeState,
    pub extend_info: Arc<RwLock<BTreeMap<String, serde_json::Value>>>,
    pub address: String,
    pub fail_access_cnt: i32,
}

impl Member {
    pub const RAFT_PORT: &str = "raftPort";
    pub const SITE_KEY: &str = "site";
    pub const AD_WEIGHT: &str = "adWeight";
    pub const WEIGHT: &str = "weight";
    pub const LAST_REFRESH_TIME: &str = "lastRefreshTime";
    pub const VERSION: &str = "version";
    pub const SUPPORT_REMOTE_C_TYPE: &str = "remoteConnectType";
    pub const READY_TO_UPGRADE: &str = "readyToUpgrade";
    pub const SUPPORT_GRAY_MODEL: &str = "supportGrayModel";

    pub const TARGET_MEMBER_CONNECT_REFUSE_ERRMSG: &str = "Connection refused";
    pub const SERVER_PORT_PROPERTY: &str = "nacos.server.main.port";
    pub const DEFAULT_SERVER_PORT: u16 = 8848;
    pub const DEFAULT_RAFT_OFFSET_PORT: u16 = 1000;
    pub const MEMBER_FAIL_ACCESS_CNT_PROPERTY: &str = "nacos.core.member.fail-access-cnt";
    pub const DEFAULT_MEMBER_FAIL_ACCESS_CNT: i16 = 3;

    pub fn calculate_raft_port(&self) -> u16 {
        self.port - Member::DEFAULT_RAFT_OFFSET_PORT
    }
}

// Builder pattern for creating Member instances
pub struct MemberBuilder {
    pub ip: String,
    pub port: u16,
    pub node_state: NodeState,
    pub extend_info: Arc<RwLock<BTreeMap<String, serde_json::Value>>>,
}

impl MemberBuilder {
    pub fn new(ip: String, port: u16) -> Self {
        let map: BTreeMap<String, Value> = BTreeMap::<String, serde_json::Value>::new();

        MemberBuilder {
            ip,
            port,
            node_state: NodeState::default(),
            extend_info: Arc::<RwLock<BTreeMap<String, serde_json::Value>>>::new(RwLock::new(map)),
        }
    }

    pub fn ip(mut self, ip: String) -> Self {
        self.ip = ip;

        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;

        self
    }

    pub fn node_state(mut self, node_state: NodeState) -> Self {
        self.node_state = node_state;

        self
    }

    pub fn build(self) -> Member {
        Member {
            ip: self.ip.clone(),
            port: self.port,
            state: self.node_state,
            extend_info: self.extend_info,
            address: format!("{}:{}", self.ip.clone(), self.port),
            fail_access_cnt: 0,
        }
    }
}
