use std::sync::LazyLock;

use actix_web::{HttpRequest, Responder, Scope, delete, get, http::StatusCode, post, put, web};
use serde::Deserialize;

use crate::model::Namespace;

use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType,
    error::{self, BatataError},
    model::{AppState, common},
    secured,
};

use batata_auth::model::ONLY_IDENTITY;

static NAMESPACE_ID_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[\w-]+").expect("Invalid regex pattern"));

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetParam {
    #[serde(alias = "namespaceId")]
    namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormData {
    #[serde(alias = "customNamespaceId")]
    custom_namespace_id: Option<String>,
    #[allow(dead_code)]
    #[serde(alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(alias = "namespaceName")]
    namespace_name: String,
    #[serde(alias = "namespaceDesc")]
    namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFormData {
    #[serde(alias = "namespaceId")]
    namespace_id: String,
    #[serde(alias = "namespaceName")]
    namespace_name: String,
    #[serde(alias = "namespaceDesc")]
    namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckParam {
    #[serde(alias = "customNamespaceId")]
    custom_namespace_id: String,
}

const NAMESPACE_ID_MAX_LENGTH: usize = 128;

#[get("")]
async fn get(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<GetParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .namespace_get_by_id(&params.namespace_id, "1")
        .await
    {
        Ok(namespace) => common::Result::<Namespace>::http_success(namespace),
        Err(err) => {
            if let Some(BatataError::ApiError(status, code, message, data)) = err.downcast_ref() {
                common::Result::<String>::http_response(
                    *status as u16,
                    *code,
                    message.to_string(),
                    data.to_string(),
                )
            } else {
                common::Result::<String>::http_response(
                    500,
                    error::SERVER_ERROR.code,
                    err.to_string(),
                    String::new(),
                )
            }
        }
    }
}

#[get("list")]
async fn find_all(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .tags(vec![ONLY_IDENTITY.to_string()])
            .build()
    );

    let mut namespaces: Vec<Namespace> = Vec::new();

    // Always include default (public) namespace first, matching Nacos behavior
    namespaces.push(Namespace::default());

    // Append custom namespaces
    for ns in data.console_datasource.namespace_find_all().await {
        if ns.namespace.is_empty() || ns.namespace == "public" {
            continue;
        }
        namespaces.push(ns);
    }

    common::Result::<Vec<Namespace>>::http_success(namespaces)
}

#[post("")]
async fn create(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<CreateFormData>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

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
        return common::Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            format!("namespaceId [{}] mismatch the pattern", namespace_id),
        );
    }

    if namespace_id.chars().count() > NAMESPACE_ID_MAX_LENGTH {
        return common::Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            format!("too long namespaceId, over {}", namespace_id),
        );
    }

    if !namespace_name_check(&namespace_name) {
        return common::Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            format!("namespaceName [{}] contains illegal char", namespace_name),
        );
    }

    let res = data
        .console_datasource
        .namespace_create(&namespace_id, &namespace_name, &namespace_desc)
        .await;

    common::Result::<bool>::http_success(res.is_ok())
}

#[put("")]
async fn update(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<UpdateFormData>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    if form.namespace_id.is_empty() {
        return common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "required parameter 'namespaceId' is missing".to_string(),
        );
    }

    if form.namespace_name.is_empty() {
        return common::Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error::PARAMETER_MISSING.code,
            error::PARAMETER_MISSING.message.to_string(),
            "required parameter 'namespaceName' is missing".to_string(),
        );
    }

    if !namespace_name_check(&form.namespace_name) {
        return common::Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error::ILLEGAL_NAMESPACE.code,
            error::ILLEGAL_NAMESPACE.message.to_string(),
            format!(
                "namespaceName [{}] contains illegal char",
                &form.namespace_name
            ),
        );
    }

    let namespace_desc = form.namespace_desc.clone().unwrap_or_default();

    match data
        .console_datasource
        .namespace_update(&form.namespace_id, &form.namespace_name, &namespace_desc)
        .await
    {
        Ok(res) => common::Result::<bool>::http_success(res),
        Err(e) => {
            tracing::error!("Failed to update namespace: {}", e);
            common::Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}

#[delete("")]
async fn delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Query<GetParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let res = data
        .console_datasource
        .namespace_delete(&form.namespace_id)
        .await;

    common::Result::<bool>::http_success(res.unwrap_or_default())
}

#[get("exist")]
async fn exist(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<CheckParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .tags(vec![ONLY_IDENTITY.to_string()])
            .build()
    );

    if params.custom_namespace_id.is_empty() {
        return common::Result::<bool>::http_success(false);
    }

    let result = data
        .console_datasource
        .namespace_check(&params.custom_namespace_id)
        .await;

    match result {
        Ok(e) => common::Result::<bool>::http_success(e),
        Err(err) => {
            if let Some(BatataError::ApiError(status, code, message, data)) = err.downcast_ref() {
                common::Result::<String>::http_response(
                    *status as u16,
                    *code,
                    message.to_string(),
                    data.to_string(),
                )
            } else {
                common::Result::<String>::http_response(
                    500,
                    error::SERVER_ERROR.code,
                    err.to_string(),
                    String::new(),
                )
            }
        }
    }
}

fn namespace_name_check(namespace_name: &str) -> bool {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[^@#$%^&*]+$").expect("Invalid regex pattern"));

    RE.is_match(namespace_name)
}

pub fn routes() -> Scope {
    web::scope("/core/namespace")
        .service(get)
        .service(find_all)
        .service(create)
        .service(update)
        .service(delete)
        .service(exist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_id_regex_valid() {
        let valid_ids = ["test-ns", "abc_123", "myNamespace", "ns1", "a-b-c_d"];
        for id in &valid_ids {
            assert!(
                NAMESPACE_ID_REGEX.is_match(id),
                "Expected '{}' to be a valid namespace ID",
                id
            );
        }
    }

    #[test]
    fn test_namespace_id_regex_invalid() {
        // Empty string should not match
        assert!(!NAMESPACE_ID_REGEX.is_match(""));
    }

    #[test]
    fn test_namespace_id_max_length() {
        // 128 chars is OK
        let valid_id: String = "a".repeat(NAMESPACE_ID_MAX_LENGTH);
        assert!(valid_id.chars().count() <= NAMESPACE_ID_MAX_LENGTH);

        // 129 chars exceeds limit
        let invalid_id: String = "a".repeat(NAMESPACE_ID_MAX_LENGTH + 1);
        assert!(invalid_id.chars().count() > NAMESPACE_ID_MAX_LENGTH);
    }

    #[test]
    fn test_namespace_name_check_valid() {
        assert!(namespace_name_check("my-namespace"));
        assert!(namespace_name_check("test_ns_123"));
        assert!(namespace_name_check("Production Environment"));
        assert!(namespace_name_check("ns-with-dashes"));
    }

    #[test]
    fn test_namespace_name_check_invalid() {
        assert!(!namespace_name_check("ns@special"));
        assert!(!namespace_name_check("ns#tag"));
        assert!(!namespace_name_check("ns$money"));
        assert!(!namespace_name_check("ns%percent"));
        assert!(!namespace_name_check("ns^caret"));
        assert!(!namespace_name_check("ns&and"));
        assert!(!namespace_name_check("ns*star"));
    }

    #[test]
    fn test_create_form_data_deserialization() {
        let json = r#"{
            "customNamespaceId": "my-ns",
            "namespaceName": "My Namespace",
            "namespaceDesc": "A test namespace"
        }"#;

        let form: CreateFormData = serde_json::from_str(json).unwrap();
        assert_eq!(form.custom_namespace_id, Some("my-ns".to_string()));
        assert_eq!(form.namespace_name, "My Namespace");
        assert_eq!(form.namespace_desc, Some("A test namespace".to_string()));
    }

    #[test]
    fn test_create_form_data_defaults() {
        let json = r#"{
            "namespaceName": "Test"
        }"#;

        let form: CreateFormData = serde_json::from_str(json).unwrap();
        assert!(form.custom_namespace_id.is_none());
        assert_eq!(form.namespace_name, "Test");
        assert!(form.namespace_desc.is_none());
    }
}
