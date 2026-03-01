// Security context and authorization macro for API access control

use std::collections::HashMap;

use actix_web::{HttpRequest, web};

use batata_auth::model::{ONLY_IDENTITY, Resource, UPDATE_PASSWORD_ENTRY_POINT};
use batata_common::SERVICE_NAME;

use crate::model::app_state::AppState;
use crate::model::constants::{DATA_ID, GROUP, GROUP_NAME, NAMESPACE_ID, TENANT};

// Re-export auth types needed by the secured! macro
// These are referenced via $crate::secured:: in the macro expansion
pub use batata_auth::model::AuthContext;
pub use batata_auth::model::CONSOLE_RESOURCE_NAME_PREFIX;
pub use batata_auth::model::GLOBAL_ADMIN_ROLE;

// Security context for API access control
#[derive(Debug, Clone)]
pub struct Secured<'a> {
    pub req: &'a HttpRequest,          // HTTP request reference
    pub data: &'a web::Data<AppState>, // Application state
    pub action: crate::ActionTypes,    // Requested action type
    pub resource: &'a str,             // Target resource name
    pub sign_type: crate::SignType,    // Service module type
    pub tags: Vec<String>,             // Security tags for permission checking
    pub api_type: crate::ApiType,      // API access type
}

impl<'a> From<&Secured<'a>> for Resource {
    fn from(val: &Secured<'a>) -> Self {
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
    pub fn builder(
        req: &'a HttpRequest,
        data: &'a web::Data<AppState>,
        resource: &'a str,
    ) -> SecuredBuilder<'a> {
        SecuredBuilder::new(req, data, resource)
    }

    pub fn has_update_password_permission(&self) -> bool {
        self.tags.iter().any(|e| e == UPDATE_PASSWORD_ENTRY_POINT)
    }

    pub fn only_identity(&self) -> bool {
        self.tags.iter().any(|e| e == ONLY_IDENTITY)
    }
}

#[derive(Debug, Clone)]
pub struct SecuredBuilder<'a> {
    req: &'a HttpRequest,
    data: &'a web::Data<AppState>,
    action: crate::ActionTypes,
    resource: &'a str,
    sign_type: crate::SignType,
    tags: Vec<String>,
    api_type: crate::ApiType,
}

impl<'a> SecuredBuilder<'a> {
    pub fn new(req: &'a HttpRequest, data: &'a web::Data<AppState>, resource: &'a str) -> Self {
        SecuredBuilder::<'a> {
            req,
            data,
            action: crate::ActionTypes::default(),
            resource,
            sign_type: crate::SignType::default(),
            tags: Vec::new(),
            api_type: crate::ApiType::default(),
        }
    }

    pub fn action(mut self, action: crate::ActionTypes) -> Self {
        self.action = action;
        self
    }

    pub fn resource(mut self, resource: &'a str) -> Self {
        self.resource = resource;
        self
    }

    pub fn sign_type(mut self, sign_type: crate::SignType) -> Self {
        self.sign_type = sign_type;
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn api_type(mut self, api_type: crate::ApiType) -> Self {
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
        let __secured = $secured;

        let __auth_enabled = __secured
            .data
            .configuration
            .auth_enabled_for_api_type(__secured.api_type);

        if __auth_enabled {
            let __skip_jwt = if __secured.api_type == $crate::ApiType::InnerApi
                || __secured.api_type == $crate::ApiType::AdminApi
            {
                let __identity_key = __secured.data.configuration.server_identity_key();
                let __identity_value = __secured.data.configuration.server_identity_value();

                let __identity_matched = if !__identity_key.is_empty() {
                    __secured
                        .req
                        .headers()
                        .get(&__identity_key)
                        .and_then(|v| v.to_str().ok())
                        .map(|v| v == __identity_value)
                        .unwrap_or(false)
                } else {
                    false
                };

                if __identity_matched {
                    true
                } else if __secured.api_type == $crate::ApiType::InnerApi
                    && !__identity_key.is_empty()
                {
                    return $crate::model::response::ErrorResult::http_response_forbidden(
                        actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                        "server identity verification failed",
                        __secured.req.path(),
                    );
                } else {
                    false
                }
            } else {
                false
            };

            if !__skip_jwt {
                let __auth_context_opt: Option<$crate::secured::AuthContext> = {
                    actix_web::HttpMessage::extensions(__secured.req)
                        .get::<$crate::secured::AuthContext>()
                        .cloned()
                };

                match __auth_context_opt {
                    None => {
                        return $crate::model::response::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                            "no auth context found",
                            __secured.req.path(),
                        );
                    }
                    Some(ref __auth_context) if !__auth_context.token_provided => {
                        return $crate::model::response::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                            "no token provided",
                            __secured.req.path(),
                        );
                    }
                    Some(ref __auth_context) if __auth_context.jwt_error.is_some() => {
                        return $crate::model::response::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                            &__auth_context.jwt_error_string(),
                            __secured.req.path(),
                        );
                    }
                    Some(ref __auth_context) => {
                        if !__secured.only_identity()
                            && !__secured.has_update_password_permission()
                        {
                            let __roles =
                                __secured.data.persistence()
                                    .role_find_by_username(
                                        &__auth_context.username,
                                    )
                                .await
                                .ok()
                                .unwrap_or_default();

                            if __roles.is_empty() {
                                return $crate::model::response::ErrorResult::http_response_forbidden(
                                    actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                                    "no roles found for user",
                                    __secured.req.path(),
                                );
                            }

                            let __global_admin = __roles
                                .iter()
                                .any(|e| e.role == $crate::secured::GLOBAL_ADMIN_ROLE);

                            if !__global_admin {
                                if __secured
                                    .resource
                                    .starts_with(
                                        $crate::secured::CONSOLE_RESOURCE_NAME_PREFIX,
                                    )
                                {
                                    return $crate::model::response::ErrorResult::http_response_forbidden(
                                        actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                                        "authorization failed!.",
                                        __secured.req.path(),
                                    );
                                }

                                let __role_names = __roles
                                    .iter()
                                    .map(|e| e.role.to_string())
                                    .collect::<Vec<String>>();
                                let __permissions =
                                    __secured.data.persistence()
                                        .permission_find_by_roles(
                                            __role_names,
                                        )
                                    .await
                                    .ok()
                                    .unwrap_or_default();

                                let __resource: batata_auth::model::Resource =
                                    if __secured.sign_type == $crate::SignType::Config {
                                        $crate::ConfigHttpResourceParser::parse(
                                            __secured.req,
                                            &__secured,
                                        )
                                    } else if __secured.sign_type == $crate::SignType::Naming {
                                        $crate::NamingHttpResourceParser::parse(
                                            __secured.req,
                                            &__secured,
                                        )
                                    } else {
                                        (&__secured).into()
                                    };

                                let __has_permission = __roles.iter().any(|__role| {
                                    __permissions
                                        .iter()
                                        .filter(|e| e.role == __role.role)
                                        .any(|e| {
                                            let mut __permission_resource =
                                                regex::escape(&e.resource).replace("\\*", ".*");

                                            if __permission_resource.starts_with(":") {
                                                __permission_resource = format!(
                                                    "{}{}",
                                                    $crate::model::constants::DEFAULT_NAMESPACE_ID,
                                                    __permission_resource,
                                                );
                                            }

                                            let __regex_match = batata_common::regex_matches(
                                                &__permission_resource,
                                                &$crate::join_resource(&__resource),
                                            );

                                            e.action.contains(__secured.action.as_str())
                                                && __regex_match
                                        })
                                });

                                if !__has_permission {
                                    return $crate::model::response::ErrorResult::http_response_forbidden(
                                        actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                                        "authorization failed!.",
                                        __secured.req.path(),
                                    );
                                }
                            }
                        }
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

pub struct NamingHttpResourceParser {}

impl NamingHttpResourceParser {
    pub fn parse(req: &HttpRequest, secured: &Secured) -> Resource {
        let namespace_id = NamingHttpResourceParser::get_namespace_id(req);
        let group = NamingHttpResourceParser::get_group(req);
        let name = NamingHttpResourceParser::get_resource_name(req);
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

        params.get(SERVICE_NAME).cloned().unwrap_or_default()
    }
}

pub fn join_resource(resource: &Resource) -> String {
    if crate::SignType::Specified.as_str() == resource.r#type {
        return resource.name.to_string();
    }

    let mut result = String::new();

    let mut namespace_id = resource.namespace_id.to_string();

    if resource.namespace_id.is_empty() {
        namespace_id = crate::model::constants::DEFAULT_NAMESPACE_ID.to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_resource_specified_type() {
        let resource = Resource {
            namespace_id: "ns".to_string(),
            group: "grp".to_string(),
            name: "custom_resource".to_string(),
            r#type: "specified".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(join_resource(&resource), "custom_resource");
    }

    #[test]
    fn test_join_resource_with_all_fields() {
        let resource = Resource {
            namespace_id: "namespace1".to_string(),
            group: "group1".to_string(),
            name: "config1".to_string(),
            r#type: "config".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(join_resource(&resource), "namespace1:group1:config/config1");
    }

    #[test]
    fn test_join_resource_empty_namespace() {
        let resource = Resource {
            namespace_id: String::new(),
            group: "group1".to_string(),
            name: "config1".to_string(),
            r#type: "config".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(join_resource(&resource), "public:group1:config/config1");
    }

    #[test]
    fn test_join_resource_empty_group() {
        let resource = Resource {
            namespace_id: "ns1".to_string(),
            group: String::new(),
            name: "config1".to_string(),
            r#type: "config".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(join_resource(&resource), "ns1:*:config/config1");
    }

    #[test]
    fn test_join_resource_empty_name() {
        let resource = Resource {
            namespace_id: "ns1".to_string(),
            group: "group1".to_string(),
            name: String::new(),
            r#type: "naming".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(join_resource(&resource), "ns1:group1:naming/*");
    }
}
