// Main library module for Batata - A Nacos-compatible service discovery and configuration management system
// This file defines core types, utilities, and security models used throughout the application

use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    str::FromStr,
};

use actix_web::{HttpRequest, web};
use if_addrs::IfAddr;

use crate::{
    auth::model::{ONLY_IDENTITY, Resource, UPDATE_PASSWORD_ENTRY_POINT},
    model::{
        common::{AppState, DATA_ID, GROUP, TENANT},
        naming::{GROUP_NAME, NAMESPACE_ID},
    },
};

// Module declarations
pub mod api; // API handlers and models
pub mod auth; // Authentication and authorization
pub mod config; // Configuration management
pub mod console; // Console web interface
pub mod core; // Core business logic
pub mod entity; // Database entities
pub mod error; // Error handling and types
pub mod metrics; // Metrics and observability
pub mod middleware; // HTTP middleware
pub mod model; // Data models and types
pub mod service; // Business services

// Action types for permission control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionTypes {
    #[default]
    Read, // Read-only access
    Write, // Write access
}

impl ActionTypes {
    // Convert action type to string representation
    pub fn as_str(self) -> &'static str {
        match self {
            ActionTypes::Read => "r",
            ActionTypes::Write => "w",
        }
    }

    // Parse string to action type
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "r" => Ok(ActionTypes::Read),
            "w" => Ok(ActionTypes::Write),
            _ => Err(format!("Invalid action: {}", s)),
        }
    }
}

impl Display for ActionTypes {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ActionTypes {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ActionTypes::from_str(s)
    }
}

// Signature types for different service modules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignType {
    #[default]
    Naming, // Service discovery module
    Config,    // Configuration management module
    Lock,      // Distributed lock module
    Ai,        // AI services module
    Console,   // Console module
    Specified, // Custom specified module
}

impl SignType {
    // Convert sign type to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            SignType::Naming => "naming",
            SignType::Config => "config",
            SignType::Lock => "lock",
            SignType::Ai => "ai",
            SignType::Console => "console",
            SignType::Specified => "specified",
        }
    }

    // Parse string to sign type
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "naming" => Ok(SignType::Naming),
            "config" => Ok(SignType::Config),
            "lock" => Ok(SignType::Lock),
            "ai" => Ok(SignType::Ai),
            "console" => Ok(SignType::Console),
            "specified" => Ok(SignType::Specified),
            _ => Err(format!("Invalid sign type: {}", s)),
        }
    }
}

impl Display for SignType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for SignType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SignType::from_str(s)
    }
}

// API access types with different permission levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ApiType {
    AdminApi,   // Administrative API with full access
    ConsoleApi, // Console management API
    #[default]
    OpenApi, // Public API for external access
    InnerApi,   // Internal service API
}

impl ApiType {
    // Get description string for API type
    pub fn description(&self) -> &'static str {
        match self {
            ApiType::AdminApi => "ADMIN_API",
            ApiType::ConsoleApi => "CONSOLE_API",
            ApiType::OpenApi => "OPEN_API",
            ApiType::InnerApi => "INNER_API",
        }
    }

    // Parse string to API type
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "ADMIN_API" => Ok(ApiType::AdminApi),
            "CONSOLE_API" => Ok(ApiType::ConsoleApi),
            "OPEN_API" => Ok(ApiType::OpenApi),
            "INNER_API" => Ok(ApiType::InnerApi),
            _ => Err(format!("Invalid API type: {}", s)),
        }
    }
}

impl Display for ApiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl FromStr for ApiType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ApiType::from_str(s)
    }
}

// Security context for API access control
#[derive(Debug, Clone)]
pub struct Secured<'a> {
    pub req: &'a HttpRequest,          // HTTP request reference
    pub data: &'a web::Data<AppState>, // Application state
    pub action: ActionTypes,           // Requested action type
    pub resource: &'a str,             // Target resource name
    pub sign_type: SignType,           // Service module type
    pub tags: Vec<String>,             // Security tags for permission checking
    pub api_type: ApiType,             // API access type
}

impl<'a> From<Secured<'a>> for Resource {
    // Convert security context to authorization resource
    fn from(val: Secured<'a>) -> Self {
        let properties = val
            .tags
            .iter()
            .map(|e| (e.to_string(), serde_json::Value::from(e.to_string())))
            .collect::<HashMap<String, serde_json::Value>>();

        Resource {
            namespace_id: String::default(),
            group: String::default(),
            name: val.resource.to_string(),
            r#type: val.sign_type.as_str().to_string(),
            properties,
        }
    }
}

impl<'a> Secured<'a> {
    // Create a new builder for Secured context
    pub fn builder(
        req: &'a HttpRequest,
        data: &'a web::Data<AppState>,
        resource: &'a str,
    ) -> SecuredBuilder<'a> {
        SecuredBuilder::new(req, data, resource)
    }

    // Check if context has password update permission
    pub fn has_update_password_permission(&self) -> bool {
        self.tags.iter().any(|e| e == UPDATE_PASSWORD_ENTRY_POINT)
    }

    // Check if context only requires identity verification
    pub fn only_identity(&self) -> bool {
        self.tags.iter().any(|e| e == ONLY_IDENTITY)
    }
}

#[derive(Debug, Clone)]
pub struct SecuredBuilder<'a> {
    req: &'a HttpRequest,
    data: &'a web::Data<AppState>,
    action: ActionTypes,
    resource: &'a str,
    sign_type: SignType,
    tags: Vec<String>,
    api_type: ApiType,
}

impl<'a> SecuredBuilder<'a> {
    pub fn new(req: &'a HttpRequest, data: &'a web::Data<AppState>, resource: &'a str) -> Self {
        SecuredBuilder::<'a> {
            req,
            data,
            action: ActionTypes::default(),
            resource,
            sign_type: SignType::default(),
            tags: Vec::new(),
            api_type: ApiType::default(),
        }
    }

    pub fn action(mut self, action: ActionTypes) -> Self {
        self.action = action;

        self
    }

    pub fn resource(mut self, resource: &'a str) -> Self {
        self.resource = resource;

        self
    }

    pub fn sign_type(mut self, sign_type: SignType) -> Self {
        self.sign_type = sign_type;

        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;

        self
    }

    pub fn api_type(mut self, api_type: ApiType) -> Self {
        self.api_type = api_type;

        self
    }

    pub fn build(self) -> Secured<'a> {
        Secured::<'a> {
            req: self.req,
            data: self.data,
            action: self.action,
            resource: self.resource,
            sign_type: self.sign_type,
            tags: self.tags,
            api_type: self.api_type,
        }
    }
}

#[macro_export]
macro_rules! secured {
    ($secured: expr) => {
        // Extract auth context before any await points to avoid holding RefCell reference
        let auth_context_opt: Option<$crate::auth::model::AuthContext> = {
            $secured
                .req
                .extensions()
                .get::<$crate::auth::model::AuthContext>()
                .cloned()
        };
        if let Some(auth_context) = auth_context_opt {
            if auth_context.jwt_error.is_some() {
                return $crate::model::common::ErrorResult::http_response_forbidden(
                    actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                    &auth_context.jwt_error_string(),
                    $secured.req.path(),
                );
            }

            if !$secured.only_identity() && !$secured.has_update_password_permission() {
                let roles = $crate::auth::service::role::find_by_username(
                    &$secured.data.database_connection,
                    &auth_context.username,
                )
                .await
                .ok()
                .unwrap_or_default();

                if roles.is_empty() {
                    return $crate::model::common::ErrorResult::http_response_forbidden(
                        actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                        &auth_context.jwt_error_string(),
                        $secured.req.path(),
                    );
                }

                let global_admin = roles
                    .iter()
                    .any(|e| e.role == $crate::auth::model::GLOBAL_ADMIN_ROLE);

                if !global_admin {
                    if $secured
                        .resource
                        .starts_with($crate::auth::model::CONSOLE_RESOURCE_NAME_PREFIX)
                    {
                        return $crate::model::common::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                            "authorization failed!.",
                            $secured.req.path(),
                        );
                    }

                    let role_names = roles
                        .iter()
                        .map(|e| e.role.to_string())
                        .collect::<Vec<String>>();
                    let permissions = $crate::auth::service::permission::find_by_roles(
                        &$secured.data.database_connection,
                        role_names,
                    )
                    .await
                    .ok()
                    .unwrap_or_default();

                    let mut resource: $crate::auth::model::Resource = $secured.into();

                    if $secured.sign_type == $crate::SignType::Config {
                        resource =
                            $crate::ConfigHttpResourceParser::parse(&$secured.req, &$secured);
                    }

                    let has_permission = roles.iter().any(|role| {
                        permissions.iter().filter(|e| e.role == role.role).any(|e| {
                            let mut permission_resource = e.resource.replace("*", ".*");

                            if permission_resource.starts_with(":") {
                                permission_resource = format!(
                                    "{}{}",
                                    $crate::model::common::DEFAULT_NAMESPACE_ID,
                                    permission_resource,
                                );
                            }

                            let regex_match = regex::Regex::new(&permission_resource)
                                .map(|re| re.is_match(&$crate::join_resource(&resource)))
                                .unwrap_or(false);

                            e.action.contains($secured.action.as_str()) && regex_match
                        })
                    });

                    if !has_permission {
                        return $crate::model::common::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                            "authorization failed!.",
                            $secured.req.path(),
                        );
                    }
                }
            }
        }
    };
}

pub struct ConfigHttpResourceParser {}

impl ConfigHttpResourceParser {
    pub fn parse(req: &HttpRequest, secured: &Secured) -> Resource {
        let namespace_id = ConfigHttpResourceParser::get_namespace_id(req);
        let group = ConfigHttpResourceParser::get_group(req);
        let name = ConfigHttpResourceParser::get_resource_name(req);
        let action = secured.action.as_str();

        let mut properties = secured
            .tags
            .iter()
            .map(|e| (e.to_string(), serde_json::Value::from(e.to_string())))
            .collect::<HashMap<String, serde_json::Value>>();

        properties.insert(
            Resource::ACTION.to_string(),
            serde_json::Value::from(action.to_string()),
        );

        Resource {
            namespace_id,
            group,
            name,
            r#type: secured.sign_type.as_str().to_string(),
            properties,
        }
    }

    pub fn get_namespace_id(req: &HttpRequest) -> String {
        let params = web::Query::<HashMap<String, String>>::from_query(req.query_string())
            .ok()
            .map(|q| q.into_inner())
            .unwrap_or_default();

        params
            .get(NAMESPACE_ID)
            .or(params.get(TENANT))
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_group(req: &HttpRequest) -> String {
        let params = web::Query::<HashMap<String, String>>::from_query(req.query_string())
            .ok()
            .map(|q| q.into_inner())
            .unwrap_or_default();

        params
            .get(GROUP_NAME)
            .or(params.get(GROUP))
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_resource_name(req: &HttpRequest) -> String {
        let params = web::Query::<HashMap<String, String>>::from_query(req.query_string())
            .ok()
            .map(|q| q.into_inner())
            .unwrap_or_default();

        params.get(DATA_ID).cloned().unwrap_or_default()
    }
}

fn join_resource(resource: &Resource) -> String {
    if SignType::Specified.as_str() == resource.r#type {
        return resource.name.to_string();
    }

    let mut result = String::new();

    let mut namespace_id = resource.namespace_id.to_string();

    if resource.namespace_id.is_empty() {
        namespace_id = crate::model::common::DEFAULT_NAMESPACE_ID.to_string();
    }

    result.push_str(namespace_id.as_str());

    let group = resource.group.to_string();

    if group.is_empty() {
        result.push_str(Resource::SPLITTER);
        result.push('*');
    } else {
        result.push_str(Resource::SPLITTER);
        result.push_str(&group);
    }

    let name = resource.name.to_string();

    if name.is_empty() {
        result.push_str(Resource::SPLITTER);
        result.push_str(&resource.r#type.to_lowercase());
        result.push_str("/*");
    } else {
        result.push_str(Resource::SPLITTER);
        result.push_str(&resource.r#type.to_lowercase());
        result.push('/');
        result.push_str(&name);
    }

    result
}

use std::sync::LazyLock;

static VALID_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new("^[a-zA-Z0-9_.:-]*$").expect("Invalid regex pattern"));

pub fn is_valid(str: &str) -> bool {
    VALID_PATTERN.is_match(str)
}

pub fn local_ip() -> String {
    if_addrs::get_if_addrs()
        .ok()
        .and_then(|addrs| {
            addrs
                .into_iter()
                .find(|iface| !iface.is_loopback() && matches!(iface.addr, IfAddr::V4(_)))
                .and_then(|iface| match iface.addr {
                    IfAddr::V4(addr) => Some(addr.ip.to_string()),
                    _ => None,
                })
        })
        .unwrap_or_else(|| "127.0.0.1".to_string())
}
