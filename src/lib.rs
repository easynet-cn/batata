use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use actix_web::{HttpRequest, web};

use crate::model::common::AppState;

pub mod console;
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

impl<'a> Secured<'a> {
    pub fn builder(
        req: &'a HttpRequest,
        data: &'a web::Data<AppState>,
        resource: &'a str,
    ) -> SecuredBuilder<'a> {
        SecuredBuilder::new(req, data, resource)
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
            } else {
                let global_admin = service::role::has_global_admin_role_by_username(
                    &$secured.data.database_connection,
                    &auth_context.username,
                )
                .await
                .ok()
                .unwrap_or_default();

                if !global_admin {
                    return crate::model::common::ErrorResult::http_response_forbidden(
                        actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                        "authorization failed!.",
                        $secured.req.path(),
                    );
                }
            }
        } else {
            return crate::model::common::ErrorResult::http_response_forbidden(
                actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                crate::model::auth::USER_NOT_FOUND_MESSAGE,
                $secured.req.path(),
            );
        }
    };
}
