use std::collections::HashMap;

use derive_builder::Builder;
use prost_types::Any;
use serde::{Deserialize, Serialize};

use crate::grpc::{Metadata, Payload};

pub trait RequestTrait {
    fn headers(&self) -> HashMap<String, String>;

    fn request_type(&self) -> String {
        String::default()
    }
    fn body(&self) -> String {
        String::default()
    }
    fn insert_headers(&mut self, headers: HashMap<String, String>);

    fn request_id(&self) -> String {
        String::default()
    }
    fn string_to_sign(&self) -> String {
        String::default()
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

    fn request_type(&self) -> String {
        todo!()
    }

    fn body(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
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
    #[serde(skip_serializing_if = "Option::is_none", flatten)]
    request: Option<Request>,
    pub module: String,
}

impl InternalRequest {
    pub fn new() -> Self {
        Self {
            request: Some(Request::new()),
            module: "internal".to_string(),
        }
    }
}

impl RequestTrait for InternalRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request
            .as_ref()
            .map(|e| e.headers())
            .unwrap_or(HashMap::<String, String>::default())
    }

    fn request_type(&self) -> String {
        String::default()
    }

    fn body(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        if let Some(request) = &mut self.request {
            request.insert_headers(headers);
        }
    }

    fn request_id(&self) -> String {
        self.request
            .as_ref()
            .map(|e| e.request_id())
            .unwrap_or_default()
    }

    fn string_to_sign(&self) -> String {
        String::default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckRequest {
    #[serde(skip_serializing_if = "Option::is_none", flatten)]
    pub internal_request: Option<InternalRequest>,
}

impl HealthCheckRequest {
    pub fn new() -> Self {
        Self {
            internal_request: Some(InternalRequest::new()),
        }
    }
}

impl RequestTrait for HealthCheckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request
            .as_ref()
            .map(|e| e.headers())
            .unwrap_or(HashMap::<String, String>::default())
    }

    fn request_type(&self) -> String {
        "HealthCheckRequest".to_string()
    }

    fn body(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        if let Some(internal_request) = &mut self.internal_request {
            internal_request.insert_headers(headers);
        }
    }

    fn request_id(&self) -> String {
        self.internal_request
            .as_ref()
            .map(|e| e.request_id())
            .unwrap_or_default()
    }

    fn string_to_sign(&self) -> String {
        String::default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResetRequest {
    #[serde(skip_serializing_if = "Option::is_none", flatten)]
    pub internal_request: Option<InternalRequest>,
    pub server_ip: String,
    pub server_port: String,
}

impl ConnectResetRequest {
    pub fn new() -> Self {
        Self {
            internal_request: Some(InternalRequest::new()),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConnectResetRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request
            .as_ref()
            .map(|e| e.headers())
            .unwrap_or(HashMap::<String, String>::default())
    }

    fn request_type(&self) -> String {
        "ConnectResetRequest".to_string()
    }

    fn body(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        if let Some(internal_request) = &mut self.internal_request {
            internal_request.insert_headers(headers);
        }
    }

    fn request_id(&self) -> String {
        self.internal_request
            .as_ref()
            .map(|e| e.request_id())
            .unwrap_or_default()
    }

    fn string_to_sign(&self) -> String {
        String::default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCheckRequest {
    #[serde(skip_serializing_if = "Option::is_none", flatten)]
    pub internal_request: Option<InternalRequest>,
}

impl ServerCheckRequest {
    pub fn new() -> Self {
        Self {
            internal_request: Some(InternalRequest::new()),
        }
    }
}

impl RequestTrait for ServerCheckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request
            .as_ref()
            .map(|e| e.headers())
            .unwrap_or(HashMap::<String, String>::default())
    }

    fn request_type(&self) -> String {
        "ServerCheckRequest".to_string()
    }

    fn body(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        if let Some(internal_request) = &mut self.internal_request {
            internal_request.insert_headers(headers);
        }
    }

    fn request_id(&self) -> String {
        self.internal_request
            .as_ref()
            .map(|e| e.request_id())
            .unwrap_or_default()
    }

    fn string_to_sign(&self) -> String {
        String::default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSetupRequest {
    #[serde(skip_serializing_if = "Option::is_none", flatten)]
    pub internal_request: Option<InternalRequest>,
    pub client_version: String,
    pub tenant: String,
    pub labels: HashMap<String, String>,
    pub client_abilities: ClientAbilities,
}

impl ConnectionSetupRequest {
    pub fn new() -> Self {
        Self {
            internal_request: Some(InternalRequest::new()),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConnectionSetupRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request
            .as_ref()
            .map(|e| e.headers())
            .unwrap_or(HashMap::<String, String>::default())
    }

    fn request_type(&self) -> String {
        "ConnectionSetupRequest".to_string()
    }

    fn body(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        if let Some(internal_request) = &mut self.internal_request {
            internal_request.insert_headers(headers);
        }
    }

    fn request_id(&self) -> String {
        self.internal_request
            .as_ref()
            .map(|e| e.request_id())
            .unwrap_or_default()
    }

    fn string_to_sign(&self) -> String {
        String::default()
    }
}

impl From<&Payload> for ConnectionSetupRequest {
    fn from(value: &Payload) -> Self {
        serde_json::from_slice::<ConnectionSetupRequest>(
            &value.body.clone().unwrap_or_default().value.to_vec(),
        )
        .unwrap_or_default()
    }
}

pub trait ResponseTrait {
    fn response_type(&self) -> String {
        String::default()
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    #[builder(default = "ResponseCode::Success.code()")]
    pub result_code: i32,
    #[builder(default)]
    pub error_code: i32,
    #[builder(default = "true")]
    pub success: bool,
    #[builder(default)]
    pub message: String,
    #[builder(default)]
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

    pub fn builder() -> ResponseBuilder {
        ResponseBuilder::default()
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    #[serde(flatten)]
    #[builder(default)]
    pub respones: Response,
}

impl HealthCheckResponse {
    pub fn new() -> Self {
        Self {
            respones: Response::new(),
        }
    }
}

impl ResponseTrait for HealthCheckResponse {
    fn response_type(&self) -> String {
        "HealthCheckResponse".to_string()
    }

    fn request_id(&mut self, request_id: String) {
        self.respones.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.respones.error_code
    }

    fn result_code(&self) -> i32 {
        self.respones.result_code
    }
}

impl Into<Any> for HealthCheckResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct ServerCheckResponse {
    #[serde(flatten)]
    #[builder(default)]
    pub respones: Response,
    #[builder(default)]
    pub connection_id: String,
    #[builder(default = "true")]
    pub support_ability_negotiation: bool,
}

impl ResponseTrait for ServerCheckResponse {
    fn response_type(&self) -> String {
        "ServerCheckResponse".to_string()
    }

    fn request_id(&mut self, request_id: String) {
        self.respones.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.respones.error_code
    }

    fn result_code(&self) -> i32 {
        self.respones.result_code
    }
}

impl Into<Any> for ServerCheckResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}
