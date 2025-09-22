use std::collections::HashMap;

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    api::model::INTERNAL_MODULE,
    grpc::{Metadata, Payload},
};

fn serialize_internal_module<S>(_: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(INTERNAL_MODULE)
}

fn deserialize_internal_module<'de, D>(_: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(INTERNAL_MODULE.to_string())
}

pub trait RequestTrait {
    fn headers(&self) -> HashMap<String, String>;

    fn request_type(&self) -> &'static str {
        ""
    }

    fn body(&self) -> Vec<u8>
    where
        Self: Serialize,
    {
        serde_json::to_vec(self).unwrap_or_default()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>);

    fn request_id(&self) -> String {
        String::default()
    }

    fn string_to_sign(&self) -> String {
        String::default()
    }

    fn from_payload<T>(value: &Payload) -> T
    where
        T: for<'a> Deserialize<'a> + Default,
    {
        serde_json::from_slice::<T>(&value.body.clone().unwrap_or_default().value.to_vec())
            .unwrap_or_default()
    }
}

pub trait ConfigRequestTrait {
    fn data_id(&self) -> String {
        String::default()
    }
    fn group_name(&self) -> String {
        String::default()
    }
    fn namespace_id(&self) -> String {
        String::default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAbilities {}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(skip)]
    pub headers: HashMap<String, String>,
    pub request_id: String,
}

impl Request {
    pub fn new() -> Self {
        Self {
            headers: HashMap::<String, String>::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for Request {
    fn headers(&self) -> HashMap<String, String> {
        self.headers.clone()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        if self.headers.is_empty() {
            self.headers = HashMap::<String, String>::with_capacity(headers.len());
        }

        for (k, v) in headers {
            self.headers.insert(k, v);
        }
    }

    fn request_id(&self) -> String {
        self.request_id.clone()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalRequest {
    #[serde(flatten)]
    pub request: Request,
    #[serde(
        serialize_with = "serialize_internal_module",
        deserialize_with = "deserialize_internal_module"
    )]
    module: String,
}

impl InternalRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for InternalRequest {
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
pub struct HealthCheckRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
}

impl HealthCheckRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl RequestTrait for HealthCheckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "HealthCheckRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for HealthCheckRequest {
    fn from(value: &Payload) -> Self {
        HealthCheckRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResetRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
    pub server_ip: String,
    pub server_port: String,
}

impl ConnectResetRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConnectResetRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConnectResetRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for ConnectResetRequest {
    fn from(value: &Payload) -> Self {
        ConnectResetRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCheckRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
}

impl ServerCheckRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl RequestTrait for ServerCheckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ServerCheckRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for ServerCheckRequest {
    fn from(value: &Payload) -> Self {
        ServerCheckRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSetupRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
    pub client_version: String,
    pub tenant: String,
    pub labels: HashMap<String, String>,
    pub client_abilities: ClientAbilities,
}

impl ConnectionSetupRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConnectionSetupRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConnectionSetupRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for ConnectionSetupRequest {
    fn from(value: &Payload) -> Self {
        ConnectionSetupRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLoaderInfoRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
}

impl ServerLoaderInfoRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ServerLoaderInfoRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ServerLoaderInfoRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for ServerLoaderInfoRequest {
    fn from(value: &Payload) -> Self {
        ServerLoaderInfoRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerReloadRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
}

impl ServerReloadRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ServerReloadRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ServerReloadRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for ServerReloadRequest {
    fn from(value: &Payload) -> Self {
        ServerReloadRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRequest {
    #[serde(flatten)]
    pub request: Request,
    #[serde(
        serialize_with = "serialize_internal_module",
        deserialize_with = "deserialize_internal_module"
    )]
    module: String,
}

impl ServerRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ServerRequest {
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
pub struct ClientDetectionRequest {
    #[serde(flatten)]
    pub server_requst: ServerRequest,
}

impl RequestTrait for ClientDetectionRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_requst.headers()
    }

    fn request_type(&self) -> &'static str {
        "ClientDetectionRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_requst.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_requst.request_id()
    }
}

impl From<&Payload> for ClientDetectionRequest {
    fn from(value: &Payload) -> Self {
        ClientDetectionRequest::from_payload(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupAckRequest {
    #[serde(flatten)]
    pub server_requst: ServerRequest,
}

impl RequestTrait for SetupAckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_requst.headers()
    }

    fn request_type(&self) -> &'static str {
        "SetupAckRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_requst.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_requst.request_id()
    }
}

impl From<&Payload> for SetupAckRequest {
    fn from(value: &Payload) -> Self {
        SetupAckRequest::from_payload(value)
    }
}

pub trait ResponseTrait {
    fn response_type(&self) -> &'static str {
        ""
    }

    fn request_id(&mut self, request_id: String);

    fn body(&self) -> Vec<u8>
    where
        Self: Serialize,
    {
        serde_json::to_vec(self).unwrap_or_default()
    }

    fn error_code(&self) -> i32 {
        ResponseCode::Success.code()
    }

    fn result_code(&self) -> i32;

    fn message(&self) -> String {
        String::default()
    }

    fn into_any(&self) -> Any
    where
        Self: Serialize,
    {
        Any {
            type_url: String::default(),
            value: self.body(),
        }
    }

    fn into_payload(&self, metadata: Option<Metadata>) -> Payload
    where
        Self: Serialize,
    {
        Payload {
            metadata: metadata,
            body: Some(self.into_any()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseCode {
    Success = 200,
    Fail = 500,
}

impl ResponseCode {
    pub fn code(&self) -> i32 {
        *self as i32
    }

    pub fn desc(&self) -> &'static str {
        match self {
            ResponseCode::Success => "Response ok",
            ResponseCode::Fail => "Response fail",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub result_code: i32,
    pub error_code: i32,
    pub success: bool,
    pub message: String,
    pub request_id: String,
}

impl Response {
    pub fn new() -> Self {
        Self {
            result_code: ResponseCode::Success.code(),
            success: true,
            ..Default::default()
        }
    }
}

impl ResponseTrait for Response {
    fn request_id(&mut self, request_id: String) {
        self.request_id = request_id
    }

    fn error_code(&self) -> i32 {
        self.error_code
    }

    fn result_code(&self) -> i32 {
        self.result_code
    }

    fn message(&self) -> String {
        self.message.clone()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl HealthCheckResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for HealthCheckResponse {
    fn response_type(&self) -> &'static str {
        "HealthCheckResponse"
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

impl Into<Any> for HealthCheckResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCheckResponse {
    #[serde(flatten)]
    pub response: Response,
    pub connection_id: String,
    pub support_ability_negotiation: bool,
}

impl ResponseTrait for ServerCheckResponse {
    fn response_type(&self) -> &'static str {
        "ServerCheckResponse"
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

impl Into<Any> for ServerCheckResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}
