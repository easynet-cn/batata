use actix_web::{get, post, put, web, HttpResponse, Responder, Scope};
use serde::Deserialize;

use crate::api::model::AppState;
use crate::common::model::RestResult;
use crate::core::model::Namespace;
use crate::service;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetNamespaceParams {
    show: Option<String>,
    namespace_id: Option<String>,
    check_namespace_id_exist: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNamespaceFormData {
    custom_namespace_id: Option<String>,
    namespace_name: String,
    namespace_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNamespaceFormData {
    namespace: String,
    namespace_show_name: String,
    namespace_desc: Option<String>,
}

const NAMESPACE_ID_MAX_LENGTH: usize = 128;

#[get("")]
pub async fn get_namespaces(
    data: web::Data<AppState>,
    params: web::Query<GetNamespaceParams>,
) -> impl Responder {
    if params.show.is_some() && params.show.as_ref().unwrap() == "all" {
        let namespace = service::namespace::get_by_namespace_id(
            data.conns.get(0).unwrap(),
            params.namespace_id.as_ref().unwrap().to_string(),
        )
        .await;

        return HttpResponse::Ok().json(namespace);
    }

    if params.check_namespace_id_exist.is_some() && params.check_namespace_id_exist.unwrap() {
        let count = service::namespace::get_count_by_tenant_id(
            data.conns.get(0).unwrap(),
            params.namespace_id.as_ref().unwrap().to_string(),
        )
        .await;

        return HttpResponse::Ok().json(count > 0);
    }

    let namespaces: Vec<Namespace> = service::namespace::find_all(data.conns.get(0).unwrap()).await;
    let rest_result = RestResult::<Vec<Namespace>>::success(namespaces);

    return HttpResponse::Ok().json(rest_result);
}

#[post("")]
pub async fn create_namespace(
    data: web::Data<AppState>,
    form: web::Form<CreateNamespaceFormData>,
) -> impl Responder {
    let namespace_id: String;

    if form.custom_namespace_id.is_some() && !form.custom_namespace_id.as_ref().unwrap().is_empty()
    {
        namespace_id = form
            .custom_namespace_id
            .as_ref()
            .unwrap()
            .trim()
            .to_string();

        let regex = regex::Regex::new(r"^[\w-]+").unwrap();

        if !regex.is_match(&namespace_id) {
            return HttpResponse::Ok().json(false);
        }

        if namespace_id.len() > NAMESPACE_ID_MAX_LENGTH {
            return HttpResponse::Ok().json(false);
        }

        if service::namespace::get_count_by_tenant_id(
            data.conns.get(0).unwrap(),
            namespace_id.clone(),
        )
        .await
            > 0
        {
            return HttpResponse::Ok().json(false);
        }
    } else {
        namespace_id = uuid::Uuid::new_v4().to_string();
    }

    let regex = regex::Regex::new(r"^[^@#$%^&*]+$").unwrap();

    if !regex.is_match(&form.namespace_name) {
        return HttpResponse::Ok().json(false);
    }

    let namespace_desc: String;

    if form.namespace_desc.is_some() {
        namespace_desc = form.namespace_desc.as_ref().unwrap().to_string();
    } else {
        namespace_desc = "".to_string();
    }

    let res = service::namespace::create(
        data.conns.get(0).unwrap(),
        namespace_id,
        form.namespace_name.clone(),
        namespace_desc,
    )
    .await;

    return HttpResponse::Ok().json(res);
}

#[put("")]
pub async fn update_namespace(
    data: web::Data<AppState>,
    form: web::Form<UpdateNamespaceFormData>,
) -> impl Responder {
    let regex = regex::Regex::new(r"^[^@#$%^&*]+$").unwrap();

    if !regex.is_match(&form.namespace_show_name) {
        return HttpResponse::Ok().json(false);
    }

    let namespace_desc: String;

    if form.namespace_desc.is_some() {
        namespace_desc = form.namespace_desc.as_ref().unwrap().to_string();
    } else {
        namespace_desc = "".to_string();
    }

    let res = service::namespace::update(
        data.conns.get(0).unwrap(),
        form.namespace.clone(),
        form.namespace_show_name.clone(),
        namespace_desc,
    )
    .await;

    return HttpResponse::Ok().json(res);
}
pub fn routers() -> Scope {
    web::scope("/namespaces")
        .service(get_namespaces)
        .service(create_namespace)
        .service(update_namespace)
}
