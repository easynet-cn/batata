use crate::api::model::AppState;
use crate::common::model::RestResult;
use crate::core::model::Namespace;
use crate::service;

use actix_web::{get, web};

#[get("")]
pub async fn get_namespaces(data: web::Data<AppState>) -> web::Json<RestResult<Vec<Namespace>>> {
    let namespaces: Vec<Namespace> = service::namespace::find_all(data.conns.get(0).unwrap()).await;
    let rest_result = RestResult::<Vec<Namespace>>::success(namespaces);

    web::Json(rest_result)
}
