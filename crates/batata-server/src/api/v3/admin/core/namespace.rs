//! V3 Admin namespace management endpoints

use std::sync::LazyLock;

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, http::StatusCode, post, put,
    web,
};
use serde::Deserialize;

use batata_config::Namespace;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    error::{self, BatataError},
    model::common::{self, AppState},
    secured, service,
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

    let namespaces: Vec<Namespace> = service::namespace::find_all(data.db()).await;

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

    match service::namespace::get_by_namespace_id(data.db(), &params.namespace_id, "1").await {
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
                HttpResponse::InternalServerError().body(err.to_string())
            }
        }
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

    let res =
        service::namespace::create(data.db(), &namespace_id, &namespace_name, &namespace_desc)
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

    match service::namespace::update(
        data.db(),
        &form.namespace_id,
        &form.namespace_name,
        &namespace_desc,
    )
    .await
    {
        Ok(res) => common::Result::<bool>::http_success(res),
        Err(e) => {
            tracing::error!("Failed to update namespace: {}", e);
            HttpResponse::InternalServerError().body(e.to_string())
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

    let res = service::namespace::delete(data.db(), &form.namespace_id).await;

    common::Result::<bool>::http_success(res.unwrap_or_default())
}

fn namespace_name_check(namespace_name: &str) -> bool {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[^@#$%^&*]+$").expect("Invalid regex pattern"));

    RE.is_match(namespace_name)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/namespace")
        .service(list_namespaces)
        .service(get_namespace)
        .service(create_namespace)
        .service(update_namespace)
        .service(delete_namespace)
}
