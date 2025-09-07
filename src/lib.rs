use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    str::FromStr,
};

use actix_web::{HttpRequest, web};

use crate::model::{
    auth::Resource,
    common::{AppState, DATA_ID, GROUP, TENANT},
    naming::{GROUP_NAME, NAMESPACE_ID},
};

pub mod api;
pub mod config;
pub mod console;
pub mod core;
pub mod entity;
pub mod error;
pub mod middleware;
pub mod model;
pub mod service;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTypes {
    Read,
    Write,
}

impl ActionTypes {
    pub fn as_str(self) -> &'static str {
        match self {
            ActionTypes::Read => "r",
            ActionTypes::Write => "w",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "r" => Ok(ActionTypes::Read),
            "w" => Ok(ActionTypes::Write),
            _ => Err(format!("Invalid action: {}", s)),
        }
    }
}

impl Default for ActionTypes {
    fn default() -> Self {
        ActionTypes::Read
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignType {
    Naming,
    Config,
    Lock,
    Ai,
    Console,
    Specified,
}

impl SignType {
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

impl Default for SignType {
    fn default() -> Self {
        SignType::Naming
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiType {
    AdminApi,
    ConsoleApi,
    OpenApi,
    InnerApi,
}

impl ApiType {
    pub fn description(&self) -> &'static str {
        match self {
            ApiType::AdminApi => "ADMIN_API",
            ApiType::ConsoleApi => "CONSOLE_API",
            ApiType::OpenApi => "OPEN_API",
            ApiType::InnerApi => "INNER_API",
        }
    }

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

impl Default for ApiType {
    fn default() -> Self {
        ApiType::OpenApi
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

#[derive(Debug, Clone)]
pub struct Secured<'a> {
    pub req: &'a HttpRequest,
    pub data: &'a web::Data<AppState>,
    pub action: ActionTypes,
    pub resource: &'a str,
    pub sign_type: SignType,
    pub tags: Vec<String>,
    pub api_type: ApiType,
}

impl<'a> Into<Resource> for Secured<'a> {
    fn into(self) -> Resource {
        let properties = self
            .tags
            .iter()
            .map(|e| (e.to_string(), serde_json::Value::from(e.to_string())))
            .collect::<HashMap<String, serde_json::Value>>();

        Resource {
            namespace_id: String::default(),
            group: String::default(),
            name: self.resource.to_string(),
            r#type: self.sign_type.as_str().to_string(),
            properties: properties,
        }
    }
}

impl<'a> Secured<'a> {
    pub fn builder(
        req: &'a HttpRequest,
        data: &'a web::Data<AppState>,
        resource: &'a str,
    ) -> SecuredBuilder<'a> {
        SecuredBuilder::new(req, data, resource)
    }

    pub fn has_update_password_permission(&self) -> bool {
        self.tags
            .iter()
            .any(|e| e == model::auth::UPDATE_PASSWORD_ENTRY_POINT)
    }

    pub fn only_identity(&self) -> bool {
        self.tags.iter().any(|e| e == model::auth::ONLY_IDENTITY)
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
            req: req,
            data: data,
            action: ActionTypes::default(),
            resource: resource,
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
        if let Some(auth_context) = $secured
            .req
            .extensions()
            .get::<crate::model::auth::AuthContext>()
            .cloned()
        {
            if auth_context.jwt_error.is_some() {
                return crate::model::common::ErrorResult::http_response_forbidden(
                    actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                    &auth_context.jwt_error_string(),
                    $secured.req.path(),
                );
            }

            if !$secured.only_identity() && !$secured.has_update_password_permission() {
                let roles = service::role::find_by_username(
                    &$secured.data.database_connection,
                    &auth_context.username,
                )
                .await
                .ok()
                .unwrap_or_default();

                if roles.is_empty() {
                    return crate::model::common::ErrorResult::http_response_forbidden(
                        actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                        &auth_context.jwt_error_string(),
                        $secured.req.path(),
                    );
                }

                let global_admin = roles
                    .iter()
                    .any(|e| e.role == crate::model::auth::GLOBAL_ADMIN_ROLE);

                if !global_admin {
                    if $secured
                        .resource
                        .starts_with(crate::model::auth::CONSOLE_RESOURCE_NAME_PREFIX)
                    {
                        return crate::model::common::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                            "authorization failed!.",
                            $secured.req.path(),
                        );
                    }

                    let role_names = roles
                        .iter()
                        .map(|e| e.role.to_string())
                        .collect::<Vec<String>>();
                    let permissions = service::permission::find_by_roles(
                        &$secured.data.database_connection,
                        role_names,
                    )
                    .await
                    .ok()
                    .unwrap_or_default();

                    let mut resource: crate::model::auth::Resource = $secured.into();

                    if $secured.sign_type == crate::SignType::Config {
                        resource = crate::ConfigHttpResourceParser::parse(&$secured.req, &$secured);
                    }

                    let has_permission = roles.iter().any(|role| {
                        permissions.iter().filter(|e| e.role == role.role).any(|e| {
                            let regex = regex::Regex::new("\\*").unwrap();

                            let mut permission_resource =
                                regex.replace_all(&e.resource, ".*").into_owned();

                            if permission_resource.starts_with(":") {
                                permission_resource = format!(
                                    "{}{}",
                                    crate::model::common::DEFAULT_NAMESPACE_ID,
                                    permission_resource,
                                );
                            }

                            let regex = regex::Regex::new(&permission_resource).unwrap();

                            e.action.contains($secured.action.as_str())
                                && regex.is_match(&crate::join_resource(&resource))
                        })
                    });

                    if !has_permission {
                        return crate::model::common::ErrorResult::http_response_forbidden(
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
            .unwrap()
            .into_inner();

        params
            .get(NAMESPACE_ID)
            .or(params.get(TENANT))
            .unwrap_or(&String::default())
            .to_string()
    }

    pub fn get_group(req: &HttpRequest) -> String {
        let params = web::Query::<HashMap<String, String>>::from_query(req.query_string())
            .ok()
            .unwrap()
            .into_inner();

        params
            .get(GROUP_NAME)
            .or(params.get(GROUP))
            .unwrap_or(&String::default())
            .to_string()
    }

    pub fn get_resource_name(req: &HttpRequest) -> String {
        let params = web::Query::<HashMap<String, String>>::from_query(req.query_string())
            .ok()
            .unwrap()
            .into_inner();

        params
            .get(DATA_ID)
            .unwrap_or(&String::default())
            .to_string()
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
        result.push_str("*");
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
        result.push_str("/");
        result.push_str(&name);
    }

    result
}

pub fn is_valid(str: &str) -> bool {
    let regex = regex::Regex::new("^[a-zA-Z0-9_.:-]*$").unwrap();

    regex.is_match(str)
}
