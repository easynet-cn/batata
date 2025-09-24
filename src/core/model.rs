use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use tokio::sync::mpsc::Sender;
use tonic::Status;

use crate::api::grpc::Payload;

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
}

impl ConnectionMeta {}

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
