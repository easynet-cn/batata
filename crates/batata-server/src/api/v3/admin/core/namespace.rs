//! V3 Admin namespace management endpoints

use std::sync::LazyLock;

use actix_web::{HttpRequest, Responder, delete, get, post, put, web};
use serde::Deserialize;

use batata_config::Namespace;

use crate::{
    ActionTypes, ApiType, Secured, SignType, error,
    model::common::{self, AppState},
    secured,
};

static NAMESPACE_ID_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[\w-]+").expect("Invalid regex pattern"));

const NAMESPACE_ID_MAX_LENGTH: usize = 128;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetParam {
    namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormData {
    custom_namespace_id: Option<String>,
    #[allow(dead_code)]
    namespace_id: Option<String>,
    namespace_name: String,
    namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFormData {
    namespace_id: String,
    namespace_name: String,
    namespace_desc: Option<String>,
}

/// GET /v3/admin/core/namespace/list
#[get("list")]
async fn list_namespaces(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let mut namespaces: Vec<Namespace> = Vec::new();

    // Always include the default (public) namespace first, matching Nacos behavior
    namespaces.push(Namespace {
        namespace: String::new(),
        namespace_show_name: "public".to_string(),
        namespace_desc: "Public Namespace".to_string(),
        quota: 200,
        config_count: 0,
        type_: 0, // GLOBAL type
    });

    // Append custom namespaces from persistence
    match data.persistence().namespace_find_all().await {
        Ok(ns_list) => {
            for ns in ns_list {
                let ns = Namespace::from(ns);
                // Skip if it's the default namespace (already added)
                if ns.namespace.is_empty() || ns.namespace == "public" {
                    continue;
                }
                namespaces.push(ns);
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list namespaces");
            return common::Result::<String>::http_internal_error(e);
        }
    };

    common::Result::<Vec<Namespace>>::http_success(namespaces)
}

/// GET /v3/admin/core/namespace
#[get("")]
async fn get_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<GetParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    match data
        .persistence()
        .namespace_get_by_id(&params.namespace_id)
        .await
    {
        Ok(Some(ns_info)) => common::Result::<Namespace>::http_success(Namespace::from(ns_info)),
        Ok(None) => common::Result::<String>::http_not_found(
            &error::NAMESPACE_NOT_EXIST,
            format!("namespace [{}] not exist", params.namespace_id),
        ),
        Err(e) => common::Result::<String>::http_internal_error(e),
    }
}

/// POST /v3/admin/core/namespace
#[post("")]
async fn create_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<CreateFormData>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
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
        return common::Result::<String>::http_not_found(
            &error::ILLEGAL_NAMESPACE,
            format!("namespaceId [{}] mismatch the pattern", namespace_id),
        );
    }

    if namespace_id.chars().count() > NAMESPACE_ID_MAX_LENGTH {
        return common::Result::<String>::http_not_found(
            &error::ILLEGAL_NAMESPACE,
            format!("too long namespaceId, over {}", namespace_id),
        );
    }

    if !namespace_name_check(&namespace_name) {
        return common::Result::<String>::http_not_found(
            &error::ILLEGAL_NAMESPACE,
            format!("namespaceName [{}] contains illegal char", namespace_name),
        );
    }

    let res = data
        .persistence()
        .namespace_create(&namespace_id, &namespace_name, &namespace_desc)
        .await;

    common::Result::<bool>::http_success(res.is_ok())
}

/// PUT /v3/admin/core/namespace
#[put("")]
async fn update_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<UpdateFormData>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    if form.namespace_id.is_empty() {
        return common::Result::<String>::http_bad_request(
            &error::PARAMETER_MISSING,
            "required parameter 'namespaceId' is missing",
        );
    }

    if form.namespace_name.is_empty() {
        return common::Result::<String>::http_bad_request(
            &error::PARAMETER_MISSING,
            "required parameter 'namespaceName' is missing",
        );
    }

    if !namespace_name_check(&form.namespace_name) {
        return common::Result::<String>::http_not_found(
            &error::ILLEGAL_NAMESPACE,
            format!(
                "namespaceName [{}] contains illegal char",
                &form.namespace_name
            ),
        );
    }

    let namespace_desc = form.namespace_desc.clone().unwrap_or_default();

    match data
        .persistence()
        .namespace_update(&form.namespace_id, &form.namespace_name, &namespace_desc)
        .await
    {
        Ok(res) => common::Result::<bool>::http_success(res),
        Err(e) => {
            tracing::error!("Failed to update namespace: {}", e);
            common::Result::<String>::http_internal_error(e)
        }
    }
}

/// DELETE /v3/admin/core/namespace
#[delete("")]
async fn delete_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Query<GetParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let res = data
        .persistence()
        .namespace_delete(&form.namespace_id)
        .await;

    common::Result::<bool>::http_success(res.unwrap_or_default())
}

/// GET /v3/admin/core/namespace/check
///
/// Check if a namespace exists. Returns the count of tenant_info records (0 or 1).
#[get("check")]
async fn check_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<GetParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/namespaces")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    match data
        .persistence()
        .namespace_check(&params.namespace_id)
        .await
    {
        Ok(exists) => common::Result::<i32>::http_success(if exists { 1 } else { 0 }),
        Err(e) => {
            tracing::error!(error = %e, "Failed to check namespace");
            common::Result::<String>::http_internal_error(e)
        }
    }
}

fn namespace_name_check(namespace_name: &str) -> bool {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[^@#$%^&*]+$").expect("Invalid regex pattern"));

    RE.is_match(namespace_name)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/namespace")
        .service(list_namespaces)
        .service(check_namespace)
        .service(get_namespace)
        .service(create_namespace)
        .service(update_namespace)
        .service(delete_namespace)
}
