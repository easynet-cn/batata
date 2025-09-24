use std::collections::{HashMap, HashSet};

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{
    api::{
        grpc::Payload,
        model::CONFIG_MODULE,
        remote::model::{Request, RequestTrait, Response, ResponseTrait, ServerRequest},
    },
    config::model::{ConfigAllInfo, ConfigHistoryInfo, ConfigInfoGrayWrapper, ConfigInfoWrapper},
};

fn serialize_config_module<S>(_: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(CONFIG_MODULE)
}

fn deserialize_config_module<'de, D>(_: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(CONFIG_MODULE.to_string())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBasicInfo {
    pub id: i64,
    pub namespace_id: String,
    pub group_name: String,
    pub data_id: String,
    pub md5: String,
    pub r#type: String,
    pub app_name: String,
    pub create_time: i64,
    pub modify_time: i64,
}

impl From<ConfigInfoWrapper> for ConfigBasicInfo {
    fn from(value: ConfigInfoWrapper) -> Self {
        Self {
            id: value.id.unwrap_or_default() as i64,
            namespace_id: value.namespace_id,
            group_name: value.group_name,
            data_id: value.data_id,
            md5: value.md5.unwrap_or_default(),
            r#type: value.r#type,
            app_name: value.app_name,
            create_time: value.create_time,
            modify_time: value.modify_time,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDetailInfo {
    #[serde(flatten)]
    pub config_basic_info: ConfigBasicInfo,
    pub content: String,
    pub desc: String,
    pub encrypted_data_key: String,
    pub create_user: String,
    pub create_ip: String,
    pub config_tags: String,
}

impl From<ConfigAllInfo> for ConfigDetailInfo {
    fn from(value: ConfigAllInfo) -> Self {
        Self {
            config_basic_info: ConfigBasicInfo {
                id: value.config_info.config_info_base.id,
                namespace_id: value.config_info.tenant,
                group_name: value.config_info.config_info_base.group,
                data_id: value.config_info.config_info_base.data_id,
                md5: value.config_info.config_info_base.md5,
                r#type: value.config_info.r#type,
                app_name: value.config_info.app_name,
                create_time: value.create_time,
                modify_time: value.modify_time,
            },
            content: value.config_info.config_info_base.content,
            desc: value.desc,
            encrypted_data_key: value.config_info.config_info_base.encrypted_data_key,
            create_user: value.create_user,
            create_ip: value.create_ip,
            config_tags: value.config_tags,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    pub config_detail_info: ConfigDetailInfo,
    pub gray_name: String,
    pub gray_rule: String,
}

impl From<ConfigInfoGrayWrapper> for ConfigGrayInfo {
    fn from(value: ConfigInfoGrayWrapper) -> Self {
        Self {
            config_detail_info: ConfigDetailInfo {
                config_basic_info: ConfigBasicInfo {
                    id: value.config_info.config_info_base.id,
                    namespace_id: value.config_info.tenant,
                    group_name: value.config_info.config_info_base.group,
                    data_id: value.config_info.config_info_base.data_id,
                    md5: value.config_info.config_info_base.md5,
                    r#type: value.config_info.r#type,
                    app_name: value.config_info.app_name,
                    create_time: 0,
                    modify_time: value.last_modified,
                },
                content: value.config_info.config_info_base.content,
                desc: "".to_string(),
                encrypted_data_key: value.config_info.config_info_base.encrypted_data_key,
                create_user: value.src_user,
                create_ip: "".to_string(),
                config_tags: "".to_string(),
            },
            gray_name: value.gray_name,
            gray_rule: value.gray_rule,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigCloneInfo {
    pub config_id: i64,
    pub target_group_name: String,
    pub target_data_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryBasicInfo {
    #[serde(flatten)]
    pub config_basic_info: ConfigBasicInfo,
    pub src_ip: String,
    pub src_user: String,
    pub op_type: String,
    pub publish_type: String,
}

impl From<ConfigHistoryInfo> for ConfigHistoryBasicInfo {
    fn from(value: ConfigHistoryInfo) -> Self {
        Self {
            config_basic_info: ConfigBasicInfo {
                id: value.id as i64,
                namespace_id: value.tenant,
                group_name: value.group,
                data_id: value.data_id,
                md5: value.md5,
                r#type: "".to_string(),
                app_name: value.app_name,
                create_time: value.created_time,
                modify_time: value.last_modified_time,
            },
            src_ip: value.src_ip,
            src_user: value.src_user,
            op_type: value.op_type,
            publish_type: value.publish_type,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetailInfo {
    #[serde(flatten)]
    pub config_history_basic_info: ConfigHistoryBasicInfo,
    pub content: String,
    pub encrypted_data_key: String,
    pub gray_name: String,
    pub ext_info: String,
}

impl From<ConfigHistoryInfo> for ConfigHistoryDetailInfo {
    fn from(value: ConfigHistoryInfo) -> Self {
        Self {
            config_history_basic_info: ConfigHistoryBasicInfo {
                config_basic_info: ConfigBasicInfo {
                    id: value.id as i64,
                    namespace_id: value.tenant,
                    group_name: value.group,
                    data_id: value.data_id,
                    md5: value.md5,
                    r#type: String::default(),
                    app_name: value.app_name,
                    create_time: value.created_time,
                    modify_time: value.last_modified_time,
                },
                src_ip: value.src_ip,
                src_user: value.src_user,
                op_type: value.op_type,
                publish_type: value.publish_type,
            },
            content: value.content,
            encrypted_data_key: value.encrypted_data_key,
            gray_name: value.gray_name,
            ext_info: value.ext_info,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenerInfo {
    pub query_type: String,
    pub listeners_status: HashMap<String, String>,
}

impl ConfigListenerInfo {
    pub const QUERY_TYPE_CONFIG: &str = "config";
    pub const QUERY_TYPE_IP: &str = "ip";
}

pub enum SameConfigPolicy {
    Abort,
    Skip,
    Overwrite,
}

impl SameConfigPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            SameConfigPolicy::Abort => "ABORT",
            SameConfigPolicy::Skip => "SKIP",
            SameConfigPolicy::Overwrite => "OVERWRITE",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "ABORT" => Ok(SameConfigPolicy::Abort),
            "SKIP" => Ok(SameConfigPolicy::Skip),
            "OVERWRITE" => Ok(SameConfigPolicy::Overwrite),
            _ => Err(format!("Invalid same config policy: {}", s)),
        }
    }
}

impl Default for SameConfigPolicy {
    fn default() -> Self {
        SameConfigPolicy::Abort
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequest {
    #[serde(flatten)]
    pub request: Request,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ConfigRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenContext {
    pub group: String,
    pub md5: String,
    pub data_id: String,
    pub tenant: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBatchListenRequest {
    pub config_request: ConfigRequest,
    pub listen: bool,
    pub config_listen_contexts: Vec<ConfigListenContext>,
}

impl ConfigBatchListenRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            listen: true,
            config_listen_contexts: Vec::new(),
        }
    }
}

impl RequestTrait for ConfigBatchListenRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.config_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigBatchListenRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.config_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.config_request.request_id()
    }
}

impl From<&Payload> for ConfigBatchListenRequest {
    fn from(value: &Payload) -> Self {
        ConfigBatchListenRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPublishRequest {
    pub config_request: ConfigRequest,
    pub content: String,
    pub cas_md5: String,
    pub addition_map: HashMap<String, String>,
}

impl ConfigPublishRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            addition_map: HashMap::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigPublishRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.config_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigPublishRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.config_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.config_request.request_id()
    }
}

impl From<&Payload> for ConfigPublishRequest {
    fn from(value: &Payload) -> Self {
        ConfigPublishRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigQueryRequest {
    pub config_request: ConfigRequest,
    pub tag: String,
}

impl ConfigQueryRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigQueryRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.config_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigQueryRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.config_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.config_request.request_id()
    }
}

impl From<&Payload> for ConfigQueryRequest {
    fn from(value: &Payload) -> Self {
        ConfigQueryRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRemoveRequest {
    pub config_request: ConfigRequest,
    pub tag: String,
}

impl ConfigRemoveRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigRemoveRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.config_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigRemoveRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.config_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.config_request.request_id()
    }
}

impl From<&Payload> for ConfigRemoveRequest {
    fn from(value: &Payload) -> Self {
        ConfigRemoveRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyWatchNotifyRequest {
    pub server_request: ServerRequest,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl FuzzyWatchNotifyRequest {
    pub fn new() -> Self {
        Self {
            server_request: ServerRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for FuzzyWatchNotifyRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_request.headers()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_request.request_id()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchChangeNotifyRequest {
    pub fuzzy_watch_notify_request: FuzzyWatchNotifyRequest,
    pub group_key: String,
    pub change_type: String,
}

impl ConfigFuzzyWatchChangeNotifyRequest {
    pub fn new() -> Self {
        Self {
            fuzzy_watch_notify_request: FuzzyWatchNotifyRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigFuzzyWatchChangeNotifyRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.fuzzy_watch_notify_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigFuzzyWatchChangeNotifyRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.fuzzy_watch_notify_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.fuzzy_watch_notify_request.request_id()
    }
}

impl From<&Payload> for ConfigFuzzyWatchChangeNotifyRequest {
    fn from(value: &Payload) -> Self {
        ConfigFuzzyWatchChangeNotifyRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub group_key: String,
    pub change_type: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchSyncRequest {
    pub fuzzy_watch_notify_request: FuzzyWatchNotifyRequest,
    pub group_key_pattern: String,
    pub sync_type: String,
    pub total_batch: i32,
    pub current_batch: i32,
    pub contexts: HashSet<Context>,
}

impl ConfigFuzzyWatchSyncRequest {
    pub fn new() -> Self {
        Self {
            fuzzy_watch_notify_request: FuzzyWatchNotifyRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigFuzzyWatchSyncRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.fuzzy_watch_notify_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigFuzzyWatchSyncRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.fuzzy_watch_notify_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.fuzzy_watch_notify_request.request_id()
    }
}

impl From<&Payload> for ConfigFuzzyWatchSyncRequest {
    fn from(value: &Payload) -> Self {
        ConfigFuzzyWatchSyncRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsKey {
    pub r#type: String,
    pub key: String,
}

impl MetricsKey {
    pub const CACHE_DATA: &str = "cacheData";
    pub const SNAPSHOT_DATA: &str = "snapshotData";

    pub fn new() -> Self {
        MetricsKey::default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfigMetricRequest {
    pub server_request: ServerRequest,
    pub metrics_keys: Vec<MetricsKey>,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ClientConfigMetricRequest {
    pub fn new() -> Self {
        Self {
            server_request: ServerRequest::new(),
            metrics_keys: Vec::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ClientConfigMetricRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ClientConfigMetricRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_request.request_id()
    }
}

impl From<&Payload> for ClientConfigMetricRequest {
    fn from(value: &Payload) -> Self {
        ClientConfigMetricRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeNotifyRequest {
    pub server_request: ServerRequest,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ConfigChangeNotifyRequest {
    pub fn new() -> Self {
        Self {
            server_request: ServerRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigChangeNotifyRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigChangeNotifyRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_request.request_id()
    }
}

impl From<&Payload> for ConfigChangeNotifyRequest {
    fn from(value: &Payload) -> Self {
        ConfigChangeNotifyRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchRequest {
    pub request: Request,
    pub group_key_pattern: String,
    pub received_group_keys: HashSet<String>,
    pub watch_type: String,
    pub is_initializing: bool,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ConfigFuzzyWatchRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigFuzzyWatchRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigChangeNotifyRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id()
    }
}

impl From<&Payload> for ConfigFuzzyWatchRequest {
    fn from(value: &Payload) -> Self {
        ConfigFuzzyWatchRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeClusterSyncRequest {
    pub config_request: ConfigRequest,
    pub last_modified: i64,
    pub gray_name: String,
}

impl ConfigChangeClusterSyncRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigChangeClusterSyncRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.config_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConfigChangeClusterSyncRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.config_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.config_request.request_id()
    }
}

impl From<&Payload> for ConfigChangeClusterSyncRequest {
    fn from(value: &Payload) -> Self {
        ConfigChangeClusterSyncRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigContext {
    pub group: String,
    pub data_id: String,
    pub tenant: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeBatchListenResponse {
    pub response: Response,
    pub changed_configs: Vec<ConfigContext>,
}

impl ConfigChangeBatchListenResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for ConfigChangeBatchListenResponse {
    fn response_type(&self) -> &'static str {
        "ConfigChangeBatchListenResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigChangeBatchListenResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPublishResponse {
    pub response: Response,
}

impl ConfigPublishResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for ConfigPublishResponse {
    fn response_type(&self) -> &'static str {
        "ConfigPublishResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigPublishResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigQueryResponse {
    pub response: Response,
    pub content: String,
    pub encrypted_data_key: String,
    pub content_type: String,
    pub md5: String,
    pub last_modified: i64,
    pub is_beta: bool,
    pub tag: String,
}

impl ConfigQueryResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for ConfigQueryResponse {
    fn response_type(&self) -> &'static str {
        "ConfigQueryResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigQueryResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRemoveResponse {
    pub response: Response,
}

impl ConfigRemoveResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for ConfigRemoveResponse {
    fn response_type(&self) -> &'static str {
        "ConfigRemoveResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigRemoveResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchChangeNotifyResponse {
    pub response: Response,
}

impl ConfigFuzzyWatchChangeNotifyResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for ConfigFuzzyWatchChangeNotifyResponse {
    fn response_type(&self) -> &'static str {
        "ConfigFuzzyWatchChangeNotifyResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigFuzzyWatchChangeNotifyResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchSyncResponse {
    pub response: Response,
}

impl ConfigFuzzyWatchSyncResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for ConfigFuzzyWatchSyncResponse {
    fn response_type(&self) -> &'static str {
        "ConfigFuzzyWatchSyncResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigFuzzyWatchSyncResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfigMetricResponse {
    pub response: Response,
    pub metrics: HashMap<String, Value>,
}

impl ClientConfigMetricResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for ClientConfigMetricResponse {
    fn response_type(&self) -> &'static str {
        "ClientConfigMetricResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ClientConfigMetricResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeNotifyResponse {
    pub response: Response,
}

impl ConfigChangeNotifyResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for ConfigChangeNotifyResponse {
    fn response_type(&self) -> &'static str {
        "ConfigChangeNotifyResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigChangeNotifyResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchResponse {
    pub response: Response,
}

impl ConfigFuzzyWatchResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for ConfigFuzzyWatchResponse {
    fn response_type(&self) -> &'static str {
        "ConfigFuzzyWatchResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigFuzzyWatchResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeClusterSyncResponse {
    pub response: Response,
}

impl ConfigChangeClusterSyncResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for ConfigChangeClusterSyncResponse {
    fn response_type(&self) -> &'static str {
        "ConfigChangeClusterSyncResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl Into<Any> for ConfigChangeClusterSyncResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}
