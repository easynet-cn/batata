// Security context and authorization macro for API access control

use std::collections::HashMap;

use actix_web::{HttpRequest, web};

use batata_auth::model::{ONLY_IDENTITY, UPDATE_PASSWORD_ENTRY_POINT};
use batata_common::SERVICE_NAME;

use crate::model::app_state::AppState;
use crate::model::constants::{DATA_ID, GROUP, GROUP_NAME, NAMESPACE_ID, TENANT};

// Re-export types needed by the secured! macro
// These are referenced via $crate::secured:: in the macro expansion
pub use batata_auth::model::CONSOLE_RESOURCE_NAME_PREFIX;
pub use batata_auth::model::GLOBAL_ADMIN_ROLE;
pub use batata_auth::model::Resource;
pub use batata_common::AuthPermission;
pub use batata_common::IdentityContext;
pub use batata_common::RequestToken;

/// Sentinel placed into an RBAC [`Resource`] when the query string cannot be
/// decoded. It is deliberately not a legal namespace/group/name so the derived
/// permission string can never match an ordinary rule.
pub const INVALID_QUERY_RESOURCE_SEGMENT: &str = "\u{0}malformed-query";

/// Detect a `%` that is not followed by two hex digits.
///
/// actix's urlencoded decoder leaves such sequences *literal* rather than
/// failing, so different layers can disagree about what the parameter value
/// is. For RBAC resource parsing that ambiguity must be a hard failure.
fn has_invalid_percent_escape(query: &str) -> bool {
    let bytes = query.as_bytes();
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && !bytes
                .get(i + 1..i + 3)
                .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit))
        {
            return true;
        }
    }
    false
}

fn parse_query_string(query: &str) -> Option<HashMap<String, String>> {
    if has_invalid_percent_escape(query) {
        tracing::warn!("Malformed percent escape in RBAC query string");
        return None;
    }
    match web::Query::<HashMap<String, String>>::from_query(query) {
        Ok(query) => Some(query.into_inner()),
        Err(e) => {
            tracing::warn!("Malformed query string in RBAC resource parse: {e}");
            None
        }
    }
}

fn parse_query_params(req: &HttpRequest) -> Option<HashMap<String, String>> {
    parse_query_string(req.query_string())
}

/// Whether `resource` was built from an undecodable query string. The
/// `secured!` macro rejects such requests instead of evaluating a permission
/// derived from empty segments.
pub fn resource_from_invalid_query(resource: &Resource) -> bool {
    resource.namespace_id == INVALID_QUERY_RESOURCE_SEGMENT
        || resource.group == INVALID_QUERY_RESOURCE_SEGMENT
        || resource.name == INVALID_QUERY_RESOURCE_SEGMENT
}

/// Security context carrying the information required for API access control.
#[derive(Debug, Clone)]
pub struct Secured<'a> {
    /// HTTP request reference.
    pub req: &'a HttpRequest,
    /// Application state.
    pub data: &'a web::Data<AppState>,
    /// Requested action type.
    pub action: crate::ActionTypes,
    /// Target resource name.
    pub resource: &'a str,
    /// Service module type.
    pub sign_type: crate::SignType,
    /// Security tags used for permission checking.
    pub tags: Vec<String>,
    /// API access type.
    pub api_type: crate::ApiType,
    /// Whether a matching server identity header may bypass user auth.
    ///
    /// `InnerApi`/`AdminApi` always accept the server identity; console-scoped
    /// endpoints only do so when explicitly opted in here. The independently
    /// deployed console reads user/role/permission data through the same
    /// `/v3/auth/**` endpoints a browser session uses, so those handlers opt in
    /// and are reached with the server identity (the console has no end-user
    /// token yet while resolving credentials during login).
    pub allow_server_identity: bool,
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
    /// Performs the `builder` operation.
    pub fn builder(
        req: &'a HttpRequest,
        data: &'a web::Data<AppState>,
        resource: &'a str,
    ) -> SecuredBuilder<'a> {
        SecuredBuilder::new(req, data, resource)
    }

    /// Performs the `has_update_password_permission` operation.
    pub fn has_update_password_permission(&self) -> bool {
        self.tags.iter().any(|e| e == UPDATE_PASSWORD_ENTRY_POINT)
    }

    /// Performs the `only_identity` operation.
    pub fn only_identity(&self) -> bool {
        self.tags.iter().any(|e| e == ONLY_IDENTITY)
    }
}

/// Builder for [`Secured`], allowing incremental configuration of the
/// security context before construction.
#[derive(Debug, Clone)]
pub struct SecuredBuilder<'a> {
    req: &'a HttpRequest,
    data: &'a web::Data<AppState>,
    action: crate::ActionTypes,
    resource: &'a str,
    sign_type: crate::SignType,
    tags: Vec<String>,
    api_type: crate::ApiType,
    allow_server_identity: bool,
}

impl<'a> SecuredBuilder<'a> {
    /// Performs the `new` operation.
    pub fn new(req: &'a HttpRequest, data: &'a web::Data<AppState>, resource: &'a str) -> Self {
        SecuredBuilder::<'a> {
            req,
            data,
            action: crate::ActionTypes::default(),
            resource,
            sign_type: crate::SignType::default(),
            tags: Vec::new(),
            api_type: crate::ApiType::default(),
            allow_server_identity: false,
        }
    }

    /// Performs the `action` operation.
    pub fn action(mut self, action: crate::ActionTypes) -> Self {
        self.action = action;
        self
    }

    /// Performs the `resource` operation.
    pub fn resource(mut self, resource: &'a str) -> Self {
        self.resource = resource;
        self
    }

    /// Performs the `sign_type` operation.
    pub fn sign_type(mut self, sign_type: crate::SignType) -> Self {
        self.sign_type = sign_type;
        self
    }

    /// Performs the `tags` operation.
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Performs the `api_type` operation.
    pub fn api_type(mut self, api_type: crate::ApiType) -> Self {
        self.api_type = api_type;
        self
    }

    /// Allow a matching server identity header to bypass user authentication.
    ///
    /// Used by console-scoped `/v3/auth/**` endpoints so an independently
    /// deployed console can read user/role/permission data over the trusted
    /// console-to-server channel.
    pub fn allow_server_identity(mut self, allow: bool) -> Self {
        self.allow_server_identity = allow;
        self
    }

    /// Performs the `build` operation.
    pub fn build(self) -> Secured<'a> {
        Secured::<'a> {
            req: self.req,
            data: self.data,
            action: self.action,
            resource: self.resource,
            sign_type: self.sign_type,
            tags: self.tags,
            api_type: self.api_type,
            allow_server_identity: self.allow_server_identity,
        }
    }
}

/// Authorization macro that delegates to `AuthPlugin` for identity validation
/// and permission checking.
///
/// Flow:
/// 1. Check if auth is enabled for the API type
/// 2. Check server identity header (InnerApi/AdminApi, or any handler that
///    opts in via `allow_server_identity`)
/// 3. Extract `RequestToken` from request extensions (populated by middleware)
/// 4. Call `auth_plugin.validate_identity()` — handles JWT decode, expiry, etc.
/// 5. For non-admin users, call `auth_plugin.validate_authority()` — checks RBAC
#[macro_export]
macro_rules! secured {
    ($secured: expr) => {
        let __secured = $secured;

        let __auth_enabled = __secured
            .data
            .configuration
            .auth_enabled_for_api_type(__secured.api_type);

        tracing::debug!(
            "[DEBUG] secured! path={}, auth_enabled={}, api_type={:?}, auth_console_enabled={}, auth_enabled={}",
            __secured.req.path(),
            __auth_enabled,
            __secured.api_type,
            __secured.data.configuration.auth_console_enabled(),
            __secured.data.configuration.auth_enabled()
        );

        if __auth_enabled {
            // Server identity check for InnerApi/AdminApi
            let __skip_auth = if __secured.api_type == $crate::ApiType::InnerApi
                || __secured.api_type == $crate::ApiType::AdminApi
                || __secured.allow_server_identity
            {
                let __identity_key = __secured.data.configuration.server_identity_key();
                let __identity_value = __secured.data.configuration.server_identity_value();

                let __identity_matched = if !__identity_key.is_empty() {
                    __secured
                        .req
                        .headers()
                        .get(&__identity_key)
                        .and_then(|v| v.to_str().ok())
                        .map(|v| bool::from($crate::subtle::ConstantTimeEq::ct_eq(v.as_bytes(), __identity_value.as_bytes())))
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

            if !__skip_auth {
                // Get the auth plugin from AppState
                let __auth_plugin = match __secured.data.auth_plugin.as_ref() {
                    Some(p) => p,
                    None => {
                        return $crate::model::response::ErrorResult::http_response_forbidden(
                            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                            "auth plugin not configured",
                            __secured.req.path(),
                        );
                    }
                };

                // Extract raw token from request extensions (populated by middleware)
                let __request_token = actix_web::HttpMessage::extensions(__secured.req)
                    .get::<$crate::secured::RequestToken>()
                    .cloned()
                    .unwrap_or_default();

                // Build IdentityContext and let the plugin validate
                let mut __identity = $crate::secured::IdentityContext {
                    token: __request_token.0,
                    ..Default::default()
                };

                let __id_result = __auth_plugin.validate_identity(&mut __identity).await;

                if !__id_result.success {
                    // Map AuthError to specific message based on error type
                    let __error_msg = match &__id_result.error {
                        Some(batata_common::AuthError::Token(
                            batata_common::TokenError::TokenMissing,
                        )) => "User not found! Please check user exist or password is right!."
                            .to_string(),
                        Some(batata_common::AuthError::Token(
                            batata_common::TokenError::TokenExpired { .. },
                        )) => "token expired!.".to_string(),
                        Some(batata_common::AuthError::Token(
                            batata_common::TokenError::TokenSignatureInvalid,
                        )) => "Invalid signature.".to_string(),
                        Some(batata_common::AuthError::Token(_)) => {
                            "token validation failed.".to_string()
                        }
                        Some(e) => e.to_string(),
                        None => "authentication failed".to_string(),
                    };

                    return $crate::model::response::Result::<String>::http_forbidden(
                        &batata_common::error::ACCESS_DENIED,
                        __error_msg,
                    );
                }

                // Store identity in request extensions so handlers can access the username
                actix_web::HttpMessage::extensions_mut(__secured.req).insert(__identity.clone());

                // Authorization check (skip for identity-only and password-update endpoints)
                if !__secured.only_identity() && !__secured.has_update_password_permission() {
                    // Admin bypass
                    if !__identity.is_global_admin {
                        // Console resources require admin
                        if __secured
                            .resource
                            .starts_with($crate::secured::CONSOLE_RESOURCE_NAME_PREFIX)
                        {
                            return $crate::model::response::ErrorResult::http_response_forbidden(
                                actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                                "authorization failed!.",
                                __secured.req.path(),
                            );
                        }

                        // Parse the resource from the request based on SignType
                        let __resource: $crate::secured::Resource =
                            if __secured.sign_type == $crate::SignType::Config {
                                $crate::ConfigHttpResourceParser::parse(__secured.req, &__secured)
                            } else if __secured.sign_type == $crate::SignType::Naming {
                                $crate::NamingHttpResourceParser::parse(__secured.req, &__secured)
                            } else {
                                (&__secured).into()
                            };

                        // A query string we could not decode must not be turned
                        // into a wildcard resource by join_resource().
                        if $crate::secured::resource_from_invalid_query(&__resource) {
                            return $crate::model::response::ErrorResult::http_response_forbidden(
                                actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                                "authorization failed!.",
                                __secured.req.path(),
                            );
                        }

                        let __permission = $crate::secured::AuthPermission {
                            resource: $crate::join_resource(&__resource),
                            action: __secured.action.as_str().to_string(),
                        };

                        let __perm_result = __auth_plugin
                            .validate_authority(&__identity, &__permission)
                            .await;

                        if !__perm_result.success {
                            let __perm_error_msg = __perm_result
                                .error
                                .as_ref()
                                .map(|e| e.to_string())
                                .unwrap_or_else(|| "authorization failed!.".to_string());

                            return $crate::model::response::Result::<String>::http_forbidden(
                                &batata_common::error::ACCESS_DENIED,
                                __perm_error_msg,
                            );
                        }
                    }
                }
            }
        }
    };
}

/// Configuration for `ConfigHttpResourceParser`.
pub struct ConfigHttpResourceParser {}

impl ConfigHttpResourceParser {
    /// Performs the `parse` operation.
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

    /// Performs the `get_namespace_id` operation.
    pub fn get_namespace_id(req: &HttpRequest) -> String {
        let Some(params) = parse_query_params(req) else {
            return INVALID_QUERY_RESOURCE_SEGMENT.to_string();
        };

        params
            .get(NAMESPACE_ID)
            .or(params.get(TENANT))
            .cloned()
            .unwrap_or_default()
    }

    /// Performs the `get_group` operation.
    pub fn get_group(req: &HttpRequest) -> String {
        let Some(params) = parse_query_params(req) else {
            return INVALID_QUERY_RESOURCE_SEGMENT.to_string();
        };

        params
            .get(GROUP_NAME)
            .or(params.get(GROUP))
            .cloned()
            .unwrap_or_default()
    }

    /// Performs the `get_resource_name` operation.
    pub fn get_resource_name(req: &HttpRequest) -> String {
        let Some(params) = parse_query_params(req) else {
            return INVALID_QUERY_RESOURCE_SEGMENT.to_string();
        };

        params.get(DATA_ID).cloned().unwrap_or_default()
    }
}

/// Parser that extracts the authorization [`Resource`] from a Naming HTTP request.
pub struct NamingHttpResourceParser {}

impl NamingHttpResourceParser {
    /// Performs the `parse` operation.
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

    /// Performs the `get_namespace_id` operation.
    pub fn get_namespace_id(req: &HttpRequest) -> String {
        let Some(params) = parse_query_params(req) else {
            return INVALID_QUERY_RESOURCE_SEGMENT.to_string();
        };

        params
            .get(NAMESPACE_ID)
            .or(params.get(TENANT))
            .cloned()
            .unwrap_or_default()
    }

    /// Performs the `get_group` operation.
    pub fn get_group(req: &HttpRequest) -> String {
        let Some(params) = parse_query_params(req) else {
            return INVALID_QUERY_RESOURCE_SEGMENT.to_string();
        };

        params
            .get(GROUP_NAME)
            .or(params.get(GROUP))
            .cloned()
            .unwrap_or_default()
    }

    /// Performs the `get_resource_name` operation.
    pub fn get_resource_name(req: &HttpRequest) -> String {
        let Some(params) = parse_query_params(req) else {
            return INVALID_QUERY_RESOURCE_SEGMENT.to_string();
        };

        params.get(SERVICE_NAME).cloned().unwrap_or_default()
    }
}

/// Performs the `join_resource` operation.
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
    use actix_web::test::TestRequest;

    #[test]
    fn malformed_query_is_not_silently_turned_into_a_wildcard_resource() {
        // `%ZZ` is an invalid percent escape. The old unwrap_or_default()
        // produced "" for every segment, which join_resource() expands to the
        // broad `public:*:config/*`.
        let req = TestRequest::with_uri("/v3/admin/cs/config?namespaceId=%ZZ&dataId=a.yml")
            .to_http_request();
        assert_eq!(
            ConfigHttpResourceParser::get_namespace_id(&req),
            INVALID_QUERY_RESOURCE_SEGMENT
        );
        assert_eq!(
            ConfigHttpResourceParser::get_resource_name(&req),
            INVALID_QUERY_RESOURCE_SEGMENT
        );
        assert_eq!(
            NamingHttpResourceParser::get_group(&req),
            INVALID_QUERY_RESOURCE_SEGMENT
        );
        let resource = Resource {
            namespace_id: ConfigHttpResourceParser::get_namespace_id(&req),
            group: ConfigHttpResourceParser::get_group(&req),
            name: ConfigHttpResourceParser::get_resource_name(&req),
            r#type: "config".to_string(),
            properties: HashMap::new(),
        };
        assert!(resource_from_invalid_query(&resource));
        assert_ne!(join_resource(&resource), "public:*:config/*");
    }

    #[test]
    fn valid_percent_encoded_query_is_accepted() {
        assert!(!has_invalid_percent_escape(
            "namespaceId=ns%201&dataId=a.yml"
        ));
        let req = TestRequest::with_uri("/v3/admin/cs/config?namespaceId=ns%201&dataId=a.yml")
            .to_http_request();
        assert_eq!(ConfigHttpResourceParser::get_namespace_id(&req), "ns 1");
        assert_eq!(ConfigHttpResourceParser::get_resource_name(&req), "a.yml");
    }

    #[test]
    fn valid_query_with_absent_optional_params_stays_empty() {
        let req = TestRequest::with_uri("/v3/admin/cs/config?dataId=a.yml").to_http_request();
        assert_eq!(ConfigHttpResourceParser::get_namespace_id(&req), "");
        assert_eq!(ConfigHttpResourceParser::get_group(&req), "");
        assert_eq!(ConfigHttpResourceParser::get_resource_name(&req), "a.yml");
        assert_eq!(NamingHttpResourceParser::get_namespace_id(&req), "");
        assert_eq!(NamingHttpResourceParser::get_resource_name(&req), "");
    }

    #[test]
    fn valid_query_is_parsed_normally() {
        let req = TestRequest::with_uri(
            "/v3/admin/ns/service?namespaceId=ns1&groupName=grp&serviceName=svc",
        )
        .to_http_request();
        assert_eq!(NamingHttpResourceParser::get_namespace_id(&req), "ns1");
        assert_eq!(NamingHttpResourceParser::get_group(&req), "grp");
        assert_eq!(NamingHttpResourceParser::get_resource_name(&req), "svc");
        assert!(!resource_from_invalid_query(&Resource {
            namespace_id: "ns1".to_string(),
            group: "grp".to_string(),
            name: "svc".to_string(),
            r#type: "naming".to_string(),
            properties: HashMap::new(),
        }));
    }

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
