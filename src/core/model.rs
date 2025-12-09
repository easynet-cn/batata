// Core data models and structures
// This file defines fundamental data structures used throughout the application

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use tokio::sync::mpsc::Sender;
use tonic::Status;

use crate::api::{
    grpc::Payload,
    model::{APP_CONN_PREFIX, APPNAME, CLIENT_VERSION_KEY},
    remote::model::{LABEL_SOURCE, LABEL_SOURCE_CLUSTER, LABEL_SOURCE_SDK},
};

// Pagination parameters for list queries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageParam {
    #[serde(default = "PageParam::default_page_no")]
    pub page_no: u64,
    #[serde(default = "PageParam::default_page_size")]
    pub page_size: u64,
}

impl PageParam {
    pub fn start(&self) -> u64 {
        (self.page_no - 1) * self.page_size
    }

    fn default_page_no() -> u64 {
        1
    }

    fn default_page_size() -> u64 {
        100
    }
}

/// Connection meta infomation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionMeta {
    /// ConnectionType.
    pub connect_type: String,

    /// Client IP Address.
    pub client_ip: String,

    /// Remote IP Address.
    pub remote_ip: String,

    /// Remote IP Port.
    pub remote_port: u16,

    /// Local Ip Port.
    pub local_port: u16,

    /// Client version.
    pub version: String,

    /// Identify Unique connectionId.
    pub connection_id: String,

    /// Create time.
    pub create_time: i64,

    /// Last active time.
    pub last_active_time: i64,

    /// App name.
    pub app_name: String,

    /// Namespace id.
    pub namespace_id: String,

    /// Labels.
    pub labels: HashMap<String, String>,

    /// Tls protected.
    pub tls_protected: bool,

    first_push_queue_block_time: i64,
    last_push_queue_block_time: i64,
}

impl ConnectionMeta {
    pub fn is_sdk_source(&self) -> bool {
        self.labels
            .get(LABEL_SOURCE)
            .is_some_and(|e| e.to_lowercase() == LABEL_SOURCE_SDK.to_lowercase())
    }

    pub fn is_cluster_source(&self) -> bool {
        self.labels
            .get(LABEL_SOURCE)
            .is_some_and(|e| e.to_lowercase() == LABEL_SOURCE_CLUSTER.to_lowercase())
    }

    pub fn get_app_labels(&self) -> HashMap<String, String> {
        let mut map = HashMap::<String, String>::new();

        map.insert(
            APPNAME.to_string(),
            self.labels
                .get(APPNAME)
                .map_or(String::default(), |e| e.to_string()),
        );
        map.insert(CLIENT_VERSION_KEY.to_string(), self.version.clone());

        for (k, v) in self.labels.iter() {
            if k.starts_with(APP_CONN_PREFIX) && k.len() > APP_CONN_PREFIX.len() && !v.is_empty() {
                map.insert(k[APP_CONN_PREFIX.len()..].to_string(), v.to_string());
            }
        }

        map
    }

    pub fn record_push_queue_block_times(&mut self) {
        if self.first_push_queue_block_time == 0 {
            self.first_push_queue_block_time = chrono::Utc::now().timestamp_millis();
        } else {
            self.last_push_queue_block_time = chrono::Utc::now().timestamp_millis();
        }
    }

    pub fn push_queue_block_times_last_over(&self, time_mills_seconds: i64) -> bool {
        self.last_push_queue_block_time - self.first_push_queue_block_time > time_mills_seconds
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub traced: bool,

    pub ability_table: HashMap<String, bool>,

    #[serde(flatten)]
    pub meta_info: ConnectionMeta,
}

impl Connection {}

pub struct GrpcClient {
    pub connection: Connection,
    pub tx: Sender<Result<Payload, Status>>,
}

impl GrpcClient {
    pub fn new(connection: Connection, tx: Sender<Result<Payload, Status>>) -> Self {
        Self { connection, tx }
    }
}
