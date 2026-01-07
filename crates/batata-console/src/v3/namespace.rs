//! Namespace management console endpoints

use std::sync::{Arc, LazyLock};

use actix_web::{HttpResponse, Responder, Scope, delete, get, http::StatusCode, post, put, web};
use serde::{Deserialize, Serialize};

use batata_config::Namespace;

use crate::datasource::ConsoleDataSource;

static NAMESPACE_ID_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[\w-]+").expect("Invalid regex pattern"));

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetParam {
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFormData {
    pub custom_namespace_id: Option<String>,
    #[allow(dead_code)]
    pub namespace_id: Option<String>,
    pub namespace_name: String,
    pub namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFormData {
    pub namespace_id: String,
    pub namespace_name: String,
    pub namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckParam {
    pub custom_namespace_id: String,
}

const NAMESPACE_ID_MAX_LENGTH: usize = 128;

/// API result wrapper
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResult<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T: Serialize> ApiResult<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data,
        }
    }

    pub fn http_success(data: T) -> HttpResponse {
        HttpResponse::Ok().json(Self::success(data))
    }

    pub fn http_response(status: u16, code: i32, message: String, data: T) -> HttpResponse {
        HttpResponse::build(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
            .json(Self {
                code,
                message,
                data,
            })
    }
}

impl ApiResult<String> {
    /// Create an internal server error response from an error
    /// This is a common pattern used across console endpoints
    pub fn http_internal_error<E: std::fmt::Display>(err: E) -> HttpResponse {
        HttpResponse::InternalServerError().json(Self {
            code: 500,
            message: "error".to_string(),
            data: err.to_string(),
        })
    }

    /// Create a bad request error response
    pub fn http_bad_request<E: std::fmt::Display>(err: E) -> HttpResponse {
        HttpResponse::BadRequest().json(Self {
            code: 400,
            message: "error".to_string(),
            data: err.to_string(),
        })
    }

    /// Create a parameter missing error response
    /// Common pattern for validating required request parameters
    pub fn http_param_missing(param_name: &str) -> HttpResponse {
        HttpResponse::BadRequest().json(Self {
            code: 10000,
            message: "parameter missing".to_string(),
            data: format!("Required parameter '{}' type String is not present", param_name),
        })
    }
}

#[get("")]
pub async fn get_namespace(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<GetParam>,
) -> impl Responder {
    match datasource.namespace_get(&params.namespace_id).await {
        Ok(namespace) => ApiResult::<Namespace>::http_success(namespace),
        Err(err) => ApiResult::http_internal_error(err),
    }
}

#[get("list")]
pub async fn list_namespaces(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    let namespaces = datasource.namespace_list().await;
    ApiResult::<Vec<Namespace>>::http_success(namespaces)
}

#[post("")]
pub async fn create_namespace(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    form: web::Form<CreateFormData>,
) -> impl Responder {
    let mut namespace_id = form
        .custom_namespace_id
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    let namespace_name = form.namespace_name.trim().to_string();
    let namespace_desc = form.namespace_desc.clone().unwrap_or_default();

    if namespace_id.is_empty() {
        namespace_id = uuid::Uuid::new_v4().to_string();
    }

    if !NAMESPACE_ID_REGEX.is_match(&namespace_id) {
        return ApiResult::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            22001,
            "illegal namespace".to_string(),
            format!("namespaceId [{}] mismatch the pattern", namespace_id),
        );
    }

    if namespace_id.chars().count() > NAMESPACE_ID_MAX_LENGTH {
        return ApiResult::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            22001,
            "illegal namespace".to_string(),
            format!("too long namespaceId, over {}", namespace_id),
        );
    }

    if !namespace_name_check(&namespace_name) {
        return ApiResult::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            22001,
            "illegal namespace".to_string(),
            format!("namespaceName [{}] contains illegal char", namespace_name),
        );
    }

    match datasource
        .namespace_create(&namespace_id, &namespace_name, &namespace_desc)
        .await
    {
        Ok(res) => ApiResult::<bool>::http_success(res),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[put("")]
pub async fn update_namespace(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    form: web::Form<UpdateFormData>,
) -> impl Responder {
    if form.namespace_id.is_empty() {
        return ApiResult::http_param_missing("namespaceId");
    }

    if form.namespace_name.is_empty() {
        return ApiResult::http_param_missing("namespaceName");
    }

    if !namespace_name_check(&form.namespace_name) {
        return ApiResult::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            22001,
            "illegal namespace".to_string(),
            format!(
                "namespaceName [{}] contains illegal char",
                &form.namespace_name
            ),
        );
    }

    let namespace_desc = form.namespace_desc.clone().unwrap_or_default();

    match datasource
        .namespace_update(&form.namespace_id, &form.namespace_name, &namespace_desc)
        .await
    {
        Ok(res) => ApiResult::<bool>::http_success(res),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[delete("")]
pub async fn delete_namespace(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    form: web::Query<GetParam>,
) -> impl Responder {
    match datasource.namespace_delete(&form.namespace_id).await {
        Ok(res) => ApiResult::<bool>::http_success(res),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

#[get("exist")]
pub async fn namespace_exists(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<CheckParam>,
) -> impl Responder {
    if params.custom_namespace_id.is_empty() {
        return ApiResult::<bool>::http_success(false);
    }

    match datasource.namespace_exists(&params.custom_namespace_id).await {
        Ok(exists) => ApiResult::<bool>::http_success(exists),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

fn namespace_name_check(namespace_name: &str) -> bool {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[^@#$%^&*]+$").expect("Invalid regex pattern"));

    RE.is_match(namespace_name)
}

pub fn routes() -> Scope {
    web::scope("/core/namespace")
        .service(get_namespace)
        .service(list_namespaces)
        .service(create_namespace)
        .service(update_namespace)
        .service(delete_namespace)
        .service(namespace_exists)
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Namespace ID regex tests ===

    #[test]
    fn test_namespace_id_regex_valid_alphanumeric() {
        assert!(NAMESPACE_ID_REGEX.is_match("abc123"));
        assert!(NAMESPACE_ID_REGEX.is_match("ABC123"));
        assert!(NAMESPACE_ID_REGEX.is_match("namespace123"));
    }

    #[test]
    fn test_namespace_id_regex_valid_with_underscore() {
        assert!(NAMESPACE_ID_REGEX.is_match("my_namespace"));
        assert!(NAMESPACE_ID_REGEX.is_match("test_env_1"));
    }

    #[test]
    fn test_namespace_id_regex_valid_with_hyphen() {
        assert!(NAMESPACE_ID_REGEX.is_match("my-namespace"));
        assert!(NAMESPACE_ID_REGEX.is_match("test-env-1"));
    }

    #[test]
    fn test_namespace_id_regex_valid_mixed() {
        assert!(NAMESPACE_ID_REGEX.is_match("my_namespace-v1"));
        assert!(NAMESPACE_ID_REGEX.is_match("prod-env_2"));
    }

    #[test]
    fn test_namespace_id_regex_uuid_format() {
        assert!(NAMESPACE_ID_REGEX.is_match("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn test_namespace_id_regex_empty_not_match() {
        // Empty string should not match as the regex requires at least one character
        assert!(!NAMESPACE_ID_REGEX.is_match(""));
    }

    // === namespace_name_check tests ===

    #[test]
    fn test_namespace_name_check_valid() {
        assert!(namespace_name_check("my namespace"));
        assert!(namespace_name_check("test-namespace"));
        assert!(namespace_name_check("namespace_1"));
        assert!(namespace_name_check("Production Environment"));
    }

    #[test]
    fn test_namespace_name_check_invalid_at() {
        assert!(!namespace_name_check("test@namespace"));
    }

    #[test]
    fn test_namespace_name_check_invalid_hash() {
        assert!(!namespace_name_check("test#namespace"));
    }

    #[test]
    fn test_namespace_name_check_invalid_dollar() {
        assert!(!namespace_name_check("test$namespace"));
    }

    #[test]
    fn test_namespace_name_check_invalid_percent() {
        assert!(!namespace_name_check("test%namespace"));
    }

    #[test]
    fn test_namespace_name_check_invalid_caret() {
        assert!(!namespace_name_check("test^namespace"));
    }

    #[test]
    fn test_namespace_name_check_invalid_ampersand() {
        assert!(!namespace_name_check("test&namespace"));
    }

    #[test]
    fn test_namespace_name_check_invalid_asterisk() {
        assert!(!namespace_name_check("test*namespace"));
    }

    #[test]
    fn test_namespace_name_check_empty() {
        // Empty string should not match
        assert!(!namespace_name_check(""));
    }

    #[test]
    fn test_namespace_name_check_unicode() {
        // Unicode characters should be allowed
        assert!(namespace_name_check("中文命名空间"));
        assert!(namespace_name_check("日本語"));
        assert!(namespace_name_check("한국어"));
    }

    #[test]
    fn test_namespace_name_check_special_allowed() {
        // Some special characters should be allowed
        assert!(namespace_name_check("namespace.name"));
        assert!(namespace_name_check("namespace:name"));
        assert!(namespace_name_check("namespace/name"));
        assert!(namespace_name_check("namespace!name"));
    }

    // === ApiResult tests ===

    #[test]
    fn test_api_result_success() {
        let result = ApiResult::success("test data".to_string());
        assert_eq!(result.code, 0);
        assert_eq!(result.message, "success");
        assert_eq!(result.data, "test data");
    }

    #[test]
    fn test_api_result_default() {
        let result: ApiResult<String> = ApiResult::default();
        assert_eq!(result.code, 0);
        assert!(result.message.is_empty());
        assert!(result.data.is_empty());
    }

    // === Form/Param tests ===

    #[test]
    fn test_get_param_deserialization() {
        let json = r#"{"namespaceId": "test-ns"}"#;
        let param: GetParam = serde_json::from_str(json).unwrap();
        assert_eq!(param.namespace_id, "test-ns");
    }

    #[test]
    fn test_create_form_data_deserialization() {
        let json = r#"{
            "customNamespaceId": "custom-id",
            "namespaceName": "Test Namespace",
            "namespaceDesc": "A test namespace"
        }"#;
        let form: CreateFormData = serde_json::from_str(json).unwrap();
        assert_eq!(form.custom_namespace_id, Some("custom-id".to_string()));
        assert_eq!(form.namespace_name, "Test Namespace");
        assert_eq!(form.namespace_desc, Some("A test namespace".to_string()));
    }

    #[test]
    fn test_update_form_data_deserialization() {
        let json = r#"{
            "namespaceId": "ns-1",
            "namespaceName": "Updated Name"
        }"#;
        let form: UpdateFormData = serde_json::from_str(json).unwrap();
        assert_eq!(form.namespace_id, "ns-1");
        assert_eq!(form.namespace_name, "Updated Name");
        assert!(form.namespace_desc.is_none());
    }

    #[test]
    fn test_check_param_deserialization() {
        let json = r#"{"customNamespaceId": "check-ns"}"#;
        let param: CheckParam = serde_json::from_str(json).unwrap();
        assert_eq!(param.custom_namespace_id, "check-ns");
    }

    #[test]
    fn test_namespace_id_max_length_constant() {
        assert_eq!(NAMESPACE_ID_MAX_LENGTH, 128);
    }
}
