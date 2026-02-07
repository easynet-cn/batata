// Main library module for Batata - A Nacos-compatible service discovery and configuration management system
// This file re-exports common types and defines security models used throughout the application

use std::collections::HashMap;

use actix_web::{HttpRequest, web};

use crate::{
    auth::model::{ONLY_IDENTITY, Resource, UPDATE_PASSWORD_ENTRY_POINT},
    model::common::{AppState, DATA_ID, GROUP, GROUP_NAME, NAMESPACE_ID, TENANT},
};

// Module declarations
pub mod api; // API handlers and models
pub mod auth; // Authentication and authorization
pub mod config; // Configuration management
pub mod console; // Console web interface
pub mod error; // Error handling and types
pub mod metrics; // Metrics and observability
pub mod middleware; // HTTP middleware
pub mod model; // Data models and types
pub mod service; // Business services
pub mod startup; // Application startup utilities

// Re-export common types from batata-common to maintain backward compatibility
pub use batata_common::{ActionTypes, ApiType, SignType, is_valid, local_ip};

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

impl<'a> From<&Secured<'a>> for Resource {
    // Convert security context reference to authorization resource
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
        // Bind to local variable to avoid re-evaluating the builder expression
        let __secured = $secured;

        // Step 1: Check if auth is enabled for this API type
        let __auth_enabled = __secured
            .data
            .configuration
            .auth_enabled_for_api_type(__secured.api_type);

        if __auth_enabled {
            // Step 2: For InnerApi, check server identity headers instead of token
            if __secured.api_type == $crate::ApiType::InnerApi {
                let __identity_key = __secured.data.configuration.server_identity_key();
                let __identity_value = __secured.data.configuration.server_identity_value();

                if !__identity_key.is_empty() {
                    let __header_match = __secured
                        .req
                        .headers()
                        .get(&__identity_key)
                        .and_then(|v| v.to_str().ok())
                        .map(|v| v == __identity_value)
                        .unwrap_or(false);

                    if !__header_match {
                        return $crate::model::common::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                            "server identity verification failed",
                            __secured.req.path(),
                        );
                    }
                }
            } else {
                // Step 3: Extract AuthContext from request extensions
                let __auth_context_opt: Option<$crate::auth::model::AuthContext> = {
                    __secured
                        .req
                        .extensions()
                        .get::<$crate::auth::model::AuthContext>()
                        .cloned()
                };

                match __auth_context_opt {
                    None => {
                        // No AuthContext at all (e.g. OPTIONS bypassed middleware)
                        return $crate::model::common::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                            "no auth context found",
                            __secured.req.path(),
                        );
                    }
                    Some(ref __auth_context) if !__auth_context.token_provided => {
                        return $crate::model::common::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                            "no token provided",
                            __secured.req.path(),
                        );
                    }
                    Some(ref __auth_context) if __auth_context.jwt_error.is_some() => {
                        return $crate::model::common::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                            &__auth_context.jwt_error_string(),
                            __secured.req.path(),
                        );
                    }
                    Some(ref __auth_context) => {
                        // Step 4: If only_identity or update_password tag, skip permission check
                        if !__secured.only_identity()
                            && !__secured.has_update_password_permission()
                        {
                            // Step 5: Look up roles
                            let __roles =
                                $crate::auth::service::role::find_by_username(
                                    __secured.data.db(),
                                    &__auth_context.username,
                                )
                                .await
                                .ok()
                                .unwrap_or_default();

                            if __roles.is_empty() {
                                return $crate::model::common::ErrorResult::http_response_forbidden(
                                    actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                                    "no roles found for user",
                                    __secured.req.path(),
                                );
                            }

                            // Step 6: If ROLE_ADMIN, pass through
                            let __global_admin = __roles
                                .iter()
                                .any(|e| e.role == $crate::auth::model::GLOBAL_ADMIN_ROLE);

                            if !__global_admin {
                                // Step 7: Console resource prefix check for non-admin
                                if __secured
                                    .resource
                                    .starts_with(
                                        $crate::auth::model::CONSOLE_RESOURCE_NAME_PREFIX,
                                    )
                                {
                                    return $crate::model::common::ErrorResult::http_response_forbidden(
                                        actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                                        "authorization failed!.",
                                        __secured.req.path(),
                                    );
                                }

                                // Step 8: Look up permissions
                                let __role_names = __roles
                                    .iter()
                                    .map(|e| e.role.to_string())
                                    .collect::<Vec<String>>();
                                let __permissions =
                                    $crate::auth::service::permission::find_by_roles(
                                        __secured.data.db(),
                                        __role_names,
                                    )
                                    .await
                                    .ok()
                                    .unwrap_or_default();

                                // Step 9: Parse resource based on sign_type
                                let __resource: $crate::auth::model::Resource =
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

                                // Step 10: Match permissions against resource + action
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
                                                    $crate::model::common::DEFAULT_NAMESPACE_ID,
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
                                    return $crate::model::common::ErrorResult::http_response_forbidden(
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
        // If auth is disabled for this API type, skip all auth checks (pass through)
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

        params
            .get(batata_common::SERVICE_NAME)
            .cloned()
            .unwrap_or_default()
    }
}

pub fn join_resource(resource: &Resource) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // join_resource tests
    // ========================================================================

    #[test]
    fn test_join_resource_specified_type() {
        let resource = auth::model::Resource {
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
        let resource = auth::model::Resource {
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
        let resource = auth::model::Resource {
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
        let resource = auth::model::Resource {
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
        let resource = auth::model::Resource {
            namespace_id: "ns1".to_string(),
            group: "group1".to_string(),
            name: String::new(),
            r#type: "naming".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(join_resource(&resource), "ns1:group1:naming/*");
    }

    #[test]
    fn test_join_resource_naming_with_all_fields() {
        let resource = auth::model::Resource {
            namespace_id: "ns1".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            name: "my-service".to_string(),
            r#type: "naming".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(
            join_resource(&resource),
            "ns1:DEFAULT_GROUP:naming/my-service"
        );
    }

    #[test]
    fn test_join_resource_naming_empty_all() {
        let resource = auth::model::Resource {
            namespace_id: String::new(),
            group: String::new(),
            name: String::new(),
            r#type: "naming".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(join_resource(&resource), "public:*:naming/*");
    }

    #[test]
    fn test_join_resource_config_empty_all() {
        let resource = auth::model::Resource {
            namespace_id: String::new(),
            group: String::new(),
            name: String::new(),
            r#type: "config".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(join_resource(&resource), "public:*:config/*");
    }

    // ========================================================================
    // NamingHttpResourceParser tests (using actix_web::test)
    // ========================================================================

    #[test]
    fn test_naming_parser_get_namespace_id() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?namespaceId=ns1")
            .to_http_request();
        assert_eq!(NamingHttpResourceParser::get_namespace_id(&req), "ns1");
    }

    #[test]
    fn test_naming_parser_get_namespace_id_from_tenant() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?tenant=tenant1")
            .to_http_request();
        assert_eq!(NamingHttpResourceParser::get_namespace_id(&req), "tenant1");
    }

    #[test]
    fn test_naming_parser_namespace_id_priority_over_tenant() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?namespaceId=ns1&tenant=tenant1")
            .to_http_request();
        assert_eq!(NamingHttpResourceParser::get_namespace_id(&req), "ns1");
    }

    #[test]
    fn test_naming_parser_get_group() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?groupName=mygroup")
            .to_http_request();
        assert_eq!(NamingHttpResourceParser::get_group(&req), "mygroup");
    }

    #[test]
    fn test_naming_parser_get_group_from_group_param() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?group=mygroup2")
            .to_http_request();
        assert_eq!(NamingHttpResourceParser::get_group(&req), "mygroup2");
    }

    #[test]
    fn test_naming_parser_get_resource_name() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?serviceName=my-service")
            .to_http_request();
        assert_eq!(
            NamingHttpResourceParser::get_resource_name(&req),
            "my-service"
        );
    }

    #[test]
    fn test_naming_parser_empty_params() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test")
            .to_http_request();
        assert_eq!(NamingHttpResourceParser::get_namespace_id(&req), "");
        assert_eq!(NamingHttpResourceParser::get_group(&req), "");
        assert_eq!(NamingHttpResourceParser::get_resource_name(&req), "");
    }

    // ========================================================================
    // ConfigHttpResourceParser tests (using actix_web::test)
    // ========================================================================

    #[test]
    fn test_config_parser_get_namespace_id() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?namespaceId=ns1")
            .to_http_request();
        assert_eq!(ConfigHttpResourceParser::get_namespace_id(&req), "ns1");
    }

    #[test]
    fn test_config_parser_get_group() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?groupName=grp1")
            .to_http_request();
        assert_eq!(ConfigHttpResourceParser::get_group(&req), "grp1");
    }

    #[test]
    fn test_config_parser_get_resource_name() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test?dataId=my-config.yaml")
            .to_http_request();
        assert_eq!(
            ConfigHttpResourceParser::get_resource_name(&req),
            "my-config.yaml"
        );
    }

    #[test]
    fn test_config_parser_empty_params() {
        let req = actix_web::test::TestRequest::default()
            .uri("/test")
            .to_http_request();
        assert_eq!(ConfigHttpResourceParser::get_namespace_id(&req), "");
        assert_eq!(ConfigHttpResourceParser::get_group(&req), "");
        assert_eq!(ConfigHttpResourceParser::get_resource_name(&req), "");
    }

    // ========================================================================
    // join_resource for naming with various combinations
    // ========================================================================

    #[test]
    fn test_join_resource_naming_empty_namespace_with_group() {
        let resource = auth::model::Resource {
            namespace_id: String::new(),
            group: "DEFAULT_GROUP".to_string(),
            name: "my-service".to_string(),
            r#type: "naming".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(
            join_resource(&resource),
            "public:DEFAULT_GROUP:naming/my-service"
        );
    }

    #[test]
    fn test_join_resource_naming_with_custom_namespace() {
        let resource = auth::model::Resource {
            namespace_id: "dev-env".to_string(),
            group: "mygroup".to_string(),
            name: "order-service".to_string(),
            r#type: "naming".to_string(),
            properties: HashMap::new(),
        };
        assert_eq!(
            join_resource(&resource),
            "dev-env:mygroup:naming/order-service"
        );
    }
}
